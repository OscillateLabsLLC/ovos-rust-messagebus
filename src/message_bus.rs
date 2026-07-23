use futures_util::{Sink, SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpSocket;
use tokio::sync::broadcast::{self, error::RecvError, error::TryRecvError, Receiver};
use tokio_tungstenite::accept_async_with_config;
use tokio_tungstenite::tungstenite::{
    error::CapacityError, protocol::WebSocketConfig, Error as WsError, Message, Utf8Bytes,
};
use tracing::{debug, error, info, trace, warn};

use crate::config::Config;

const TCP_BACKLOG: u32 = 1024;
const WRITE_BATCH_SIZE: usize = 64;
const WRITE_BATCH_BYTES_LIMIT: usize = 256 * 1024;

#[derive(Clone)]
pub struct MessageBus {
    config: Arc<Config>,
    message_tx: broadcast::Sender<Utf8Bytes>,
}

impl MessageBus {
    pub fn new(config: Config) -> Self {
        // broadcast::channel panics on zero capacity; clamp so a bad config can't crash startup
        let (message_tx, _) = broadcast::channel(config.message_buffer_capacity.max(1));
        Self {
            config: Arc::new(config),
            message_tx,
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port).parse()?;
        let socket = if addr.is_ipv6() {
            TcpSocket::new_v6()?
        } else {
            TcpSocket::new_v4()?
        };
        socket.set_reuseaddr(true)?;
        socket.bind(addr)?;
        let listener = socket.listen(TCP_BACKLOG)?;
        info!(
            "MessageBus listening on {} (route: {})",
            addr, self.config.route
        );

        while let Ok((stream, _)) = listener.accept().await {
            let bus_clone = self.clone();
            tokio::spawn(async move {
                if let Err(e) = bus_clone.handle_connection(stream).await {
                    let msg = e.to_string();
                    if msg.contains("Handshake not finished") {
                        debug!("Connection closed before WebSocket handshake (likely a healthcheck probe)");
                    } else {
                        error!("Error handling connection: {}", e);
                    }
                }
            });
        }

        Ok(())
    }

    async fn handle_connection(
        &self,
        stream: tokio::net::TcpStream,
    ) -> Result<(), Box<dyn std::error::Error>> {
        stream.set_nodelay(true)?;
        let ws_stream = accept_async_with_config(stream, Some(self.websocket_config())).await?;
        let (mut write, mut read) = ws_stream.split();
        let mut rx = self.message_tx.subscribe();

        debug!(
            "WebSocket connection opened (subscribers: {})",
            self.message_tx.receiver_count()
        );

        // Send the OVOS "connected" greeting expected by all bus clients
        let greeting = r#"{"type": "connected", "data": {}, "context": {"session": {"session_id": "default"}}}"#;
        if let Err(e) = write.send(Message::Text(greeting.into())).await {
            error!("Failed to send greeting: {}", e);
            return Ok(());
        }

        let read_bus = self.clone();
        let mut read_handle = tokio::spawn(async move {
            while let Some(message) = read.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        trace!("Received message: {}", text);
                        let _ = read_bus.message_tx.send(text);
                    }
                    Ok(Message::Close(_)) => {
                        debug!("WebSocket connection closed");
                        break;
                    }
                    Ok(_) => {}
                    Err(WsError::Capacity(CapacityError::MessageTooLong { size, max_size })) => {
                        error!(
                            "Message too large: {} bytes exceeds {} byte limit (max_msg_size: {} MB)",
                            size, max_size, read_bus.config.max_msg_size
                        );
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
        });

        let mut write_handle = tokio::spawn(async move {
            loop {
                match flush_next_batch(
                    &mut write,
                    &mut rx,
                    WRITE_BATCH_SIZE,
                    WRITE_BATCH_BYTES_LIMIT,
                )
                .await
                {
                    Ok(BatchState::Continue) => {}
                    Ok(BatchState::SlowConsumer(skipped)) => {
                        warn!(
                            "Slow consumer lagged, dropped {} messages — disconnecting",
                            skipped
                        );
                        break;
                    }
                    Ok(BatchState::Closed) => {
                        break;
                    }
                    Err(e) => {
                        error!("Error sending message: {}", e);
                        break;
                    }
                }
            }
        });

        tokio::select! {
            _ = &mut read_handle => {
                write_handle.abort();
                let _ = write_handle.await;
            },
            _ = &mut write_handle => {
                read_handle.abort();
                let _ = read_handle.await;
            },
        }

        Ok(())
    }

    fn websocket_config(&self) -> WebSocketConfig {
        let max_message_size = self.config.max_msg_size as usize * 1024 * 1024;
        WebSocketConfig::default()
            .max_message_size(Some(max_message_size))
            .max_frame_size(Some(max_message_size))
    }
}

#[derive(Debug)]
enum BatchState {
    Continue,
    SlowConsumer(u64),
    Closed,
}

async fn flush_next_batch<S>(
    write: &mut S,
    rx: &mut Receiver<Utf8Bytes>,
    batch_size: usize,
    batch_bytes_limit: usize,
) -> Result<BatchState, S::Error>
where
    S: Sink<Message> + Unpin,
{
    let first = match rx.recv().await {
        Ok(message) => message,
        Err(RecvError::Lagged(skipped)) => return Ok(BatchState::SlowConsumer(skipped)),
        Err(RecvError::Closed) => return Ok(BatchState::Closed),
    };

    let mut batched_bytes = first.len();
    write.feed(Message::Text(first)).await?;
    let mut batched_messages = 1;

    let mut close_state = BatchState::Continue;
    while batched_messages < batch_size && batched_bytes < batch_bytes_limit {
        match rx.try_recv() {
            Ok(message) => {
                batched_bytes += message.len();
                write.feed(Message::Text(message)).await?;
                batched_messages += 1;
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Lagged(skipped)) => {
                close_state = BatchState::SlowConsumer(skipped);
                break;
            }
            Err(TryRecvError::Closed) => {
                close_state = BatchState::Closed;
                break;
            }
        }
    }

    write.flush().await?;
    Ok(close_state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::pin::Pin;
    use std::task::{Context as TaskContext, Poll};

    fn test_config(message_buffer_capacity: usize, max_msg_size: u32) -> Config {
        Config {
            host: "127.0.0.1".to_string(),
            port: 8181,
            route: "/core".to_string(),
            ssl: false,
            max_msg_size,
            message_buffer_capacity,
            extra: HashMap::new(),
        }
    }

    #[derive(Default)]
    struct VecSink {
        messages: Vec<Message>,
        flushes: usize,
    }

    impl Sink<Message> for VecSink {
        type Error = std::convert::Infallible;

        fn poll_ready(
            self: Pin<&mut Self>,
            _: &mut TaskContext<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
            self.messages.push(item);
            Ok(())
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            _: &mut TaskContext<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            self.flushes += 1;
            Poll::Ready(Ok(()))
        }

        fn poll_close(
            self: Pin<&mut Self>,
            _: &mut TaskContext<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    struct FailingSink;

    impl Sink<Message> for FailingSink {
        type Error = &'static str;

        fn poll_ready(
            self: Pin<&mut Self>,
            _: &mut TaskContext<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: Pin<&mut Self>, _: Message) -> Result<(), Self::Error> {
            Err("sink write failed")
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _: &mut TaskContext<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(
            self: Pin<&mut Self>,
            _: &mut TaskContext<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    fn sent_texts(sink: &VecSink) -> Vec<String> {
        sink.messages
            .iter()
            .map(|m| match m {
                Message::Text(t) => t.to_string(),
                other => panic!("unexpected non-text message: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn websocket_config_converts_megabytes() {
        let bus = MessageBus::new(test_config(1024, 25));
        let ws_config = bus.websocket_config();
        assert_eq!(ws_config.max_message_size, Some(25 * 1024 * 1024));
        assert_eq!(ws_config.max_frame_size, Some(25 * 1024 * 1024));
    }

    #[test]
    fn zero_buffer_capacity_does_not_panic() {
        let bus = MessageBus::new(test_config(0, 25));
        assert_eq!(bus.message_tx.receiver_count(), 0);
    }

    #[tokio::test]
    async fn flushes_single_message() {
        let (tx, mut rx) = broadcast::channel::<Utf8Bytes>(16);
        let mut sink = VecSink::default();
        tx.send("hello".into()).unwrap();

        let state = flush_next_batch(&mut sink, &mut rx, 64, 1024)
            .await
            .unwrap();

        assert!(matches!(state, BatchState::Continue));
        assert_eq!(sent_texts(&sink), vec!["hello"]);
        assert_eq!(sink.flushes, 1);
    }

    #[tokio::test]
    async fn batches_queued_messages_up_to_batch_size() {
        let (tx, mut rx) = broadcast::channel::<Utf8Bytes>(16);
        let mut sink = VecSink::default();
        for i in 0..5 {
            tx.send(format!("msg-{i}").into()).unwrap();
        }

        let state = flush_next_batch(&mut sink, &mut rx, 3, 1024 * 1024)
            .await
            .unwrap();

        assert!(matches!(state, BatchState::Continue));
        assert_eq!(sent_texts(&sink), vec!["msg-0", "msg-1", "msg-2"]);
        assert_eq!(sink.flushes, 1);

        // The remaining messages are picked up by the next batch
        let state = flush_next_batch(&mut sink, &mut rx, 3, 1024 * 1024)
            .await
            .unwrap();
        assert!(matches!(state, BatchState::Continue));
        assert_eq!(sink.messages.len(), 5);
    }

    #[tokio::test]
    async fn stops_batching_at_byte_limit() {
        let (tx, mut rx) = broadcast::channel::<Utf8Bytes>(16);
        let mut sink = VecSink::default();
        for _ in 0..3 {
            tx.send("aaaa".into()).unwrap();
        }

        // First message (4 bytes) is under the 5-byte limit, second pushes past it
        let state = flush_next_batch(&mut sink, &mut rx, 64, 5).await.unwrap();

        assert!(matches!(state, BatchState::Continue));
        assert_eq!(sink.messages.len(), 2);
    }

    #[tokio::test]
    async fn stops_batching_when_byte_limit_exactly_reached() {
        let (tx, mut rx) = broadcast::channel::<Utf8Bytes>(16);
        let mut sink = VecSink::default();
        tx.send("aaaa".into()).unwrap();
        tx.send("aaaa".into()).unwrap();

        // First message lands exactly on the 4-byte limit, so batching must stop
        let state = flush_next_batch(&mut sink, &mut rx, 64, 4).await.unwrap();

        assert!(matches!(state, BatchState::Continue));
        assert_eq!(sink.messages.len(), 1);
    }

    #[tokio::test]
    async fn first_message_always_sent_even_over_byte_limit() {
        let (tx, mut rx) = broadcast::channel::<Utf8Bytes>(16);
        let mut sink = VecSink::default();
        tx.send("a large message".into()).unwrap();
        tx.send("second".into()).unwrap();

        let state = flush_next_batch(&mut sink, &mut rx, 64, 1).await.unwrap();

        assert!(matches!(state, BatchState::Continue));
        assert_eq!(sent_texts(&sink), vec!["a large message"]);
    }

    #[tokio::test]
    async fn lagged_receiver_reports_slow_consumer() {
        let (tx, mut rx) = broadcast::channel::<Utf8Bytes>(2);
        let mut sink = VecSink::default();
        for i in 0..4 {
            tx.send(format!("msg-{i}").into()).unwrap();
        }

        let state = flush_next_batch(&mut sink, &mut rx, 64, 1024)
            .await
            .unwrap();

        match state {
            BatchState::SlowConsumer(skipped) => assert_eq!(skipped, 2),
            _ => panic!("expected SlowConsumer"),
        }
        assert!(sink.messages.is_empty());
    }

    #[tokio::test]
    async fn closed_channel_drains_remaining_then_reports_closed() {
        let (tx, mut rx) = broadcast::channel::<Utf8Bytes>(16);
        let mut sink = VecSink::default();
        tx.send("first".into()).unwrap();
        tx.send("second".into()).unwrap();
        drop(tx);

        let state = flush_next_batch(&mut sink, &mut rx, 64, 1024)
            .await
            .unwrap();

        assert!(matches!(state, BatchState::Closed));
        assert_eq!(sent_texts(&sink), vec!["first", "second"]);
        assert_eq!(sink.flushes, 1);
    }

    #[tokio::test]
    async fn closed_channel_with_no_messages_reports_closed() {
        let (tx, mut rx) = broadcast::channel::<Utf8Bytes>(16);
        let mut sink = VecSink::default();
        drop(tx);

        let state = flush_next_batch(&mut sink, &mut rx, 64, 1024)
            .await
            .unwrap();

        assert!(matches!(state, BatchState::Closed));
        assert!(sink.messages.is_empty());
    }

    #[tokio::test]
    async fn sink_error_is_propagated() {
        let (tx, mut rx) = broadcast::channel::<Utf8Bytes>(16);
        let mut sink = FailingSink;
        tx.send("hello".into()).unwrap();

        let result = flush_next_batch(&mut sink, &mut rx, 64, 1024).await;

        assert_eq!(result.unwrap_err(), "sink write failed");
    }
}

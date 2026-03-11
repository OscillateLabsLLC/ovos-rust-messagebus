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
        let (message_tx, _) = broadcast::channel(config.message_buffer_capacity);
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
                    error!("Error handling connection: {}", e);
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

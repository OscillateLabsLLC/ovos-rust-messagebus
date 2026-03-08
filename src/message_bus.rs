use futures_util::{Sink, SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast::{self, error::RecvError, error::TryRecvError, Receiver};
use tokio_tungstenite::accept_async_with_config;
use tokio_tungstenite::tungstenite::{protocol::WebSocketConfig, Message, Utf8Bytes};

use crate::config::Config;

const MESSAGE_BUFFER_CAPACITY: usize = 1024;
const WRITE_BATCH_SIZE: usize = 64;
const WRITE_BATCH_BYTES_LIMIT: usize = 256 * 1024;

#[derive(Clone)]
pub struct MessageBus {
    config: Arc<Config>,
    message_tx: broadcast::Sender<Utf8Bytes>,
}

impl MessageBus {
    pub fn new(config: Config) -> Self {
        let (message_tx, _) = broadcast::channel(MESSAGE_BUFFER_CAPACITY);
        Self {
            config: Arc::new(config),
            message_tx,
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = TcpListener::bind(&addr).await?;
        println!(
            "MessageBus listening on {} (route: {})",
            addr, self.config.route
        );

        while let Ok((stream, _)) = listener.accept().await {
            let bus_clone = self.clone();
            tokio::spawn(async move {
                if let Err(e) = bus_clone.handle_connection(stream).await {
                    eprintln!("Error handling connection: {}", e);
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

        let read_bus = self.clone();
        let read_handle = tokio::spawn(async move {
            while let Some(message) = read.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        let _ = read_bus.broadcast_message(text);
                    }
                    Ok(Message::Close(_)) => {
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
        });

        let write_handle = tokio::spawn(async move {
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
                    Ok(BatchState::CloseSlowConsumer(_)) => {
                        break;
                    }
                    Ok(BatchState::Closed) => {
                        break;
                    }
                    Err(e) => {
                        eprintln!("Error sending message: {}", e);
                        break;
                    }
                }
            }
        });

        let mut read_handle = read_handle;
        let mut write_handle = write_handle;
        tokio::select! {
            _ = &mut read_handle => {
                write_handle.abort();
            },
            _ = &mut write_handle => {
                read_handle.abort();
            },
        }
        let _ = read_handle.await;
        let _ = write_handle.await;

        Ok(())
    }

    fn broadcast_message(
        &self,
        message: Utf8Bytes,
    ) -> Result<usize, broadcast::error::SendError<Utf8Bytes>> {
        self.message_tx.send(message)
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
    CloseSlowConsumer(u64),
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
        Err(RecvError::Lagged(skipped)) => return Ok(BatchState::CloseSlowConsumer(skipped)),
        Err(RecvError::Closed) => return Ok(BatchState::Closed),
    };

    let mut batched_messages = 1;
    let mut batched_bytes = first.len();
    write.feed(Message::Text(first)).await?;

    let mut close_state = BatchState::Continue;
    while batched_messages < batch_size && batched_bytes < batch_bytes_limit {
        match rx.try_recv() {
            Ok(message) => {
                batched_messages += 1;
                batched_bytes += message.len();
                write.feed(Message::Text(message)).await?;
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Lagged(skipped)) => {
                close_state = BatchState::CloseSlowConsumer(skipped);
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

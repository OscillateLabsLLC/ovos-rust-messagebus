use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::accept_async_with_config;
use tokio_tungstenite::tungstenite::protocol::{Message, WebSocketConfig};
use tracing::{debug, error, info, trace, warn};

use crate::config::Config;

const MESSAGE_BUFFER_CAPACITY: usize = 1024;

#[derive(Clone)]
pub struct MessageBus {
    config: Arc<Config>,
    message_tx: broadcast::Sender<Message>,
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

        debug!("WebSocket connection opened (subscribers: {})", self.message_tx.receiver_count());

        let read_bus = self.clone();
        let read_handle = tokio::spawn(async move {
            while let Some(message) = read.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        trace!("Received message: {}", text);
                        let _ = read_bus.message_tx.send(Message::Text(text));
                    }
                    Ok(Message::Close(_)) => {
                        debug!("WebSocket connection closed");
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
        });

        let write_handle = tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(message) => {
                        if let Err(e) = write.send(message).await {
                            error!("Error sending message: {}", e);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!("Slow consumer lagged, dropped {} messages — disconnecting", skipped);
                        break;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
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

    fn websocket_config(&self) -> WebSocketConfig {
        let max_message_size = self.config.max_msg_size as usize * 1024 * 1024;
        let mut config = WebSocketConfig::default();
        config.max_message_size = Some(max_message_size);
        config.max_frame_size = Some(max_message_size);
        config
    }
}

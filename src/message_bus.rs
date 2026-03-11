use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_tungstenite::accept_async_with_config;
use tokio_tungstenite::tungstenite::protocol::{Message, WebSocketConfig};
use tracing::{debug, error, info, trace};

use crate::config::Config;

#[derive(Clone)]
pub struct MessageBus {
    config: Arc<Config>,
    connections: Arc<Mutex<Vec<UnboundedSender<Message>>>>,
}

impl MessageBus {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            connections: Arc::new(Mutex::new(Vec::new())),
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
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx_clone = tx.clone();
        {
            let mut connections = self.connections.lock().unwrap();
            connections.push(tx);
        }
        debug!("WebSocket connection opened (total: {})", self.connections.lock().unwrap().len());

        let (mut write, mut read) = ws_stream.split();

        let read_bus = self.clone();
        let read_handle = tokio::spawn(async move {
            while let Some(message) = read.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        trace!("Received message: {}", text);
                        read_bus.broadcast_message(&text).await;
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
            read_bus.remove_connection(&tx_clone).await;
        });

        let write_handle = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if let Err(e) = write.send(message).await {
                    error!("Error sending message: {}", e);
                    break;
                }
            }
        });

        tokio::select! {
            _ = read_handle => {},
            _ = write_handle => {},
        }

        Ok(())
    }

    async fn broadcast_message(&self, message: &str) {
        let mut connections = self.connections.lock().unwrap();
        connections.retain(|tx| tx.send(Message::Text(message.to_string())).is_ok());
    }

    async fn remove_connection(&self, tx: &UnboundedSender<Message>) {
        let mut connections = self.connections.lock().unwrap();
        connections.retain(|conn| !conn.same_channel(tx));
    }

    fn websocket_config(&self) -> WebSocketConfig {
        let max_message_size = self.config.max_msg_size as usize * 1024 * 1024;
        let mut config = WebSocketConfig::default();
        config.max_message_size = Some(max_message_size);
        config.max_frame_size = Some(max_message_size);
        config
    }
}

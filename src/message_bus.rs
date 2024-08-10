use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::config::Config;
use EventEmitter::EventEmitter;

#[derive(Clone)]
pub struct MessageBus {
    config: Arc<Config>,
    connections: Arc<Mutex<Vec<UnboundedSender<Message>>>>,
    event_emitter: Arc<Mutex<EventEmitter>>,
}

impl MessageBus {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            connections: Arc::new(Mutex::new(Vec::new())),
            event_emitter: Arc::new(Mutex::new(EventEmitter::new())),
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
        let ws_stream = accept_async(stream).await?;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx_clone = tx.clone();
        {
            let mut connections = self.connections.lock().unwrap();
            connections.push(tx);
        }

        let (mut write, mut read) = ws_stream.split();

        let read_bus = self.clone();
        let read_handle = tokio::spawn(async move {
            while let Some(message) = read.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        if text.len() as u32 > read_bus.config.max_msg_size * 1024 * 1024 {
                            eprintln!("Message size exceeds maximum allowed size");
                            break;
                        }
                        println!("Received message: {}", text);
                        read_bus.broadcast_message(&text).await;
                        let event_emitter = read_bus.event_emitter.lock().unwrap();
                        event_emitter.emit(&text);
                    }
                    Ok(Message::Close(_)) => {
                        println!("WebSocket connection closed");
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
            read_bus.remove_connection(&tx_clone).await;
        });

        let write_handle = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if let Err(e) = write.send(message).await {
                    eprintln!("Error sending message: {}", e);
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
        connections.retain(|tx| match tx.send(Message::Text(message.to_string())) {
            Ok(_) => true,
            Err(_) => false,
        });
    }

    async fn remove_connection(&self, tx: &UnboundedSender<Message>) {
        let mut connections = self.connections.lock().unwrap();
        connections.retain(|conn| !conn.same_channel(tx));
    }
}

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use EventEmitter::EventEmitter;

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    host: String,
    port: u16,
}

#[derive(Clone)]
struct MessageBus {
    connections: Arc<Mutex<Vec<UnboundedSender<Message>>>>,
    event_emitter: Arc<Mutex<EventEmitter>>,
}

impl MessageBus {
    fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(Vec::new())),
            event_emitter: Arc::new(Mutex::new(EventEmitter::new())),
        }
    }

    async fn handle_connection(
        &self,
        ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    ) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        {
            let mut connections = self.connections.lock().unwrap();
            connections.push(tx);
        }

        let (mut write, mut read) = ws_stream.split();

        let read_bus = self.clone();
        tokio::spawn(async move {
            while let Some(message) = read.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        println!("Received message: {}", text);
                        let connections = read_bus.connections.lock().unwrap();
                        for conn in connections.iter() {
                            if let Err(e) = conn.send(Message::Text(text.clone())) {
                                eprintln!("Failed to send message: {}", e);
                            }
                        }
                        let event_emitter = read_bus.event_emitter.lock().unwrap();
                        event_emitter.emit(&text);
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
            // Handle connection close
            let mut connections = read_bus.connections.lock().unwrap();
            connections.retain(|conn| !conn.is_closed());
        });

        while let Some(message) = rx.recv().await {
            if let Err(e) = write.send(message).await {
                eprintln!("Failed to send message: {}", e);
                break;
            }
        }

        // Clean up connections after the write loop ends
        let mut connections = self.connections.lock().unwrap();
        connections.retain(|conn| !conn.is_closed());
    }

    async fn run(&self, config: Config) {
        let listener = TcpListener::bind(format!("{}:{}", config.host, config.port))
            .await
            .unwrap();
        while let Ok((stream, _)) = listener.accept().await {
            let ws_stream = accept_async(stream).await.unwrap();
            let bus_clone = self.clone();
            tokio::spawn(async move {
                bus_clone.handle_connection(ws_stream).await;
            });
        }
    }
}

#[tokio::main]
async fn main() {
    let config = Config {
        host: "0.0.0.0".to_string(),
        port: 8181,
    };

    let message_bus = MessageBus::new();
    message_bus.run(config).await;
}

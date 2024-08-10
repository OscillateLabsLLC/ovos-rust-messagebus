use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use EventEmitter::EventEmitter;

use crate::config::Config;

#[derive(Clone)]
pub struct MessageBus {
    config: Arc<Config>,
    connections: Arc<Mutex<Vec<UnboundedSender<Message>>>>,
    event_emitter: Arc<Mutex<EventEmitter>>,
}

impl MessageBus {
    pub async fn run(&self) {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = TcpListener::bind(&addr)
            .await
            .expect("Failed to bind to address");
        println!(
            "MessageBus listening on {} (route: {})",
            addr, self.config.route
        );

        while let Ok((stream, _)) = listener.accept().await {
            let bus_clone = self.clone();
            tokio::spawn(async move {
                match accept_async(stream).await {
                    Ok(ws_stream) => {
                        bus_clone.handle_connection(ws_stream).await;
                    }
                    Err(e) => {
                        eprintln!("Error during WebSocket handshake: {}", e);
                    }
                }
            });
        }
    }

    pub async fn run(&self) {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = TcpListener::bind(&addr).await.unwrap();
        println!(
            "MessageBus listening on {} (route: {})",
            addr, self.config.route
        );

        while let Ok((stream, _)) = listener.accept().await {
            let ws_stream = accept_async(stream).await.unwrap();
            let bus_clone = self.clone();
            tokio::spawn(async move {
                bus_clone.handle_connection(ws_stream).await;
            });
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
                        if text.len() as u32 > read_bus.config.max_msg_size * 1024 * 1024 {
                            eprintln!("Message size exceeds maximum allowed size");
                            break;
                        }
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
}

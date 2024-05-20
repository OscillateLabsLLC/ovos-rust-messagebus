use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
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
    event_emitter: Arc<Mutex<EventEmitter>>,
}

impl MessageBus {
    fn new() -> Self {
        Self {
            event_emitter: Arc::new(Mutex::new(EventEmitter::new())),
        }
    }

    async fn handle_connection(
        &self,
        mut ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    ) {
        let event_emitter = self.event_emitter.clone();

        while let Some(message) = ws_stream.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    let event_emitter = event_emitter.lock().unwrap();
                    event_emitter.emit("message_received");
                    println!("Received message: {}", text);
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("WebSocket error: {}", e);
                    break;
                }
            }
        }
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
        host: "127.0.0.1".to_string(),
        port: 8765,
    };

    let message_bus = MessageBus::new();
    message_bus.run(config).await;
}

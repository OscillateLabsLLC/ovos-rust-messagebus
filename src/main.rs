use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use simd_json::value::OwnedValue;
use simd_json::ValueAccess;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use EventEmitter::EventEmitter;

const FILTER_TYPES: &[&str] = &["gui.status.request", "gui.page.upload"];

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    host: String,
    port: u16,
}

#[derive(Clone)]
struct MessageBus {
    connections: Arc<RwLock<HashMap<usize, UnboundedSender<Message>>>>,
    event_emitter: Arc<RwLock<EventEmitter>>,
}

impl MessageBus {
    fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            event_emitter: Arc::new(RwLock::new(EventEmitter::new())),
        }
    }

    async fn handle_connection(
        bus: Arc<MessageBus>,
        ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        connection_id: usize,
    ) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        {
            let mut connections = bus.connections.write().unwrap();
            connections.insert(connection_id, tx);
        }

        let (mut write, mut read) = ws_stream.split();

        let event_handler =
            MessageBusEventHandler::new(bus.connections.clone(), bus.event_emitter.clone());

        let origin = ""; // Get the origin from the WebSocket handshake
        if !event_handler.check_origin(origin) {
            // Close the connection if the origin is not allowed
            let _ = write.send(Message::Close(None)).await;
            return;
        }

        event_handler.on_open(connection_id).await;

        let read_event_handler = event_handler.clone();
        tokio::spawn(async move {
            while let Some(message) = read.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        read_event_handler
                            .on_message(Message::Text(text), connection_id)
                            .await;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
            read_event_handler.on_close(connection_id).await;
        });

        while let Some(message) = rx.recv().await {
            if let Err(e) = write.send(message).await {
                eprintln!("Failed to send message: {}", e);
                break;
            }
        }

        event_handler.on_close(connection_id).await;
    }

    async fn run(&self, config: Config) {
        let listener = TcpListener::bind(format!("{}:{}", config.host, config.port))
            .await
            .unwrap();
        let mut connection_id = 0;
        while let Ok((stream, _)) = listener.accept().await {
            let ws_stream = accept_async(stream).await.unwrap();
            let bus = Arc::new(self.clone());
            let current_connection_id = connection_id;
            connection_id += 1;
            tokio::spawn(async move {
                MessageBus::handle_connection(bus, ws_stream, current_connection_id).await;
            });
        }
    }
}

#[derive(Clone)]
struct MessageBusEventHandler {
    connections: Arc<RwLock<HashMap<usize, UnboundedSender<Message>>>>,
    event_emitter: Arc<RwLock<EventEmitter>>,
}

impl MessageBusEventHandler {
    fn new(
        connections: Arc<RwLock<HashMap<usize, UnboundedSender<Message>>>>,
        event_emitter: Arc<RwLock<EventEmitter>>,
    ) -> Self {
        Self {
            connections,
            event_emitter,
        }
    }

    fn check_origin(&self, _origin: &str) -> bool {
        true
    }

    async fn on_message(&self, message: Message, _connection_id: usize) {
        if let Message::Text(mut text) = message {
            println!("Received message: {}", text);

            // Parse the JSON payload using simd-json
            let json_value: OwnedValue = simd_json::serde::from_str(text.as_mut_str()).unwrap();

            // Extract the message type from the JSON value
            let msg_type_value = json_value.get("msg_type");
            let msg_type = msg_type_value.as_str().unwrap_or("");

            // Check if the message type should be filtered out
            if !FILTER_TYPES.contains(&msg_type) {
                // Serialize the response back to JSON using simd-json
                let response = simd_json::serde::to_string(&json_value).unwrap();

                self.emit(Message::Text(response));

                let event_emitter = self.event_emitter.read().unwrap();
                event_emitter.emit(&text);
            }
        }
    }

    async fn on_open(&self, _connection_id: usize) {
        let message = Message::Text(
            r#"{"msg_type": "connected", "context": {"session": {"session_id": "default"}}}"#
                .to_string(),
        );
        self.emit(message);
    }

    async fn on_close(&self, connection_id: usize) {
        let mut connections = self.connections.write().unwrap();
        connections.remove(&connection_id);
    }

    fn emit(&self, message: Message) {
        let connections = self.connections.read().unwrap();
        for (_, conn) in connections.iter() {
            if let Err(e) = conn.send(message.clone()) {
                eprintln!("Failed to send message: {}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let config = Config {
        host: "127.0.0.1".to_string(),
        port: 8181,
    };

    let message_bus = MessageBus::new();
    message_bus.run(config).await;
}

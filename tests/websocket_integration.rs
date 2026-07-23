use futures_util::{SinkExt, StreamExt};
use ovos_messagebus::{Config, MessageBus};
use std::collections::HashMap;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

const RECV_TIMEOUT: Duration = Duration::from_secs(5);

type WsClient = WebSocketStream<MaybeTlsStream<TcpStream>>;

fn test_config(port: u16, max_msg_size: u32) -> Config {
    Config {
        host: "127.0.0.1".to_string(),
        port,
        route: "/core".to_string(),
        ssl: false,
        max_msg_size,
        message_buffer_capacity: 1024,
        extra: HashMap::new(),
    }
}

/// Bind to port 0 to find a free port, release it, and hand it to the bus.
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind");
    listener.local_addr().expect("no local addr").port()
}

async fn start_bus(max_msg_size: u32) -> u16 {
    let port = free_port();
    let bus = MessageBus::new(test_config(port, max_msg_size));
    tokio::spawn(async move {
        let _ = bus.run().await;
    });
    port
}

async fn connect_client(port: u16) -> WsClient {
    let url = format!("ws://127.0.0.1:{port}/core");
    for _ in 0..50 {
        if let Ok((ws, _)) = connect_async(&url).await {
            return ws;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("could not connect to message bus on port {port}");
}

async fn next_text(ws: &mut WsClient) -> String {
    loop {
        let msg = timeout(RECV_TIMEOUT, ws.next())
            .await
            .expect("timed out waiting for message")
            .expect("stream ended unexpectedly")
            .expect("websocket error");
        if let Message::Text(text) = msg {
            return text.to_string();
        }
    }
}

/// Connect and consume the "connected" greeting so tests start from a clean stream.
async fn connect_and_greet(port: u16) -> WsClient {
    let mut ws = connect_client(port).await;
    let greeting = next_text(&mut ws).await;
    assert!(greeting.contains("connected"), "expected greeting first");
    ws
}

#[tokio::test]
async fn sends_conformant_greeting_on_connect() {
    let port = start_bus(25).await;
    let mut ws = connect_client(port).await;

    let greeting = next_text(&mut ws).await;
    let parsed: serde_json::Value = serde_json::from_str(&greeting).expect("greeting is not JSON");

    assert_eq!(parsed["type"], "connected");
    assert!(parsed["data"].is_object());
    assert_eq!(parsed["context"]["session"]["session_id"], "default");
}

#[tokio::test]
async fn broadcasts_to_all_clients_including_sender() {
    let port = start_bus(25).await;
    let mut sender = connect_and_greet(port).await;
    let mut receiver_a = connect_and_greet(port).await;
    let mut receiver_b = connect_and_greet(port).await;

    let payload = r#"{"type": "speak", "data": {"utterance": "hello"}, "context": {}}"#;
    sender.send(Message::Text(payload.into())).await.unwrap();

    assert_eq!(next_text(&mut sender).await, payload);
    assert_eq!(next_text(&mut receiver_a).await, payload);
    assert_eq!(next_text(&mut receiver_b).await, payload);
}

#[tokio::test]
async fn preserves_message_order() {
    let port = start_bus(25).await;
    let mut sender = connect_and_greet(port).await;
    let mut receiver = connect_and_greet(port).await;

    for i in 0..20 {
        let msg = format!(r#"{{"type": "test.message", "data": {{"n": {i}}}}}"#);
        sender.send(Message::Text(msg.into())).await.unwrap();
    }

    for i in 0..20 {
        let received = next_text(&mut receiver).await;
        let parsed: serde_json::Value = serde_json::from_str(&received).unwrap();
        assert_eq!(parsed["data"]["n"], i, "message {i} out of order");
    }
}

#[tokio::test]
async fn client_disconnect_does_not_stop_the_bus() {
    let port = start_bus(25).await;

    let mut first = connect_and_greet(port).await;
    first.close(None).await.unwrap();

    let mut second = connect_and_greet(port).await;
    let payload = r#"{"type": "ping", "data": {}}"#;
    second.send(Message::Text(payload.into())).await.unwrap();

    assert_eq!(next_text(&mut second).await, payload);
}

#[tokio::test]
async fn ignores_binary_messages() {
    let port = start_bus(25).await;
    let mut sender = connect_and_greet(port).await;
    let mut receiver = connect_and_greet(port).await;

    sender
        .send(Message::Binary(vec![1, 2, 3].into()))
        .await
        .unwrap();
    let payload = r#"{"type": "after.binary", "data": {}}"#;
    sender.send(Message::Text(payload.into())).await.unwrap();

    // The binary frame is dropped; the next thing the receiver sees is the text message
    assert_eq!(next_text(&mut receiver).await, payload);
}

#[tokio::test]
async fn oversized_message_disconnects_sender_without_broadcast() {
    let port = start_bus(1).await; // 1 MB limit
    let mut sender = connect_and_greet(port).await;
    let mut receiver = connect_and_greet(port).await;

    let oversized = format!(
        r#"{{"type": "too.big", "data": {{"blob": "{}"}}}}"#,
        "x".repeat(2 * 1024 * 1024)
    );
    let _ = sender.send(Message::Text(oversized.into())).await;

    // The server drops the offending connection...
    let disconnected = timeout(RECV_TIMEOUT, async {
        loop {
            match sender.next().await {
                None | Some(Err(_)) => break,
                Some(Ok(Message::Close(_))) => break,
                Some(Ok(_)) => {}
            }
        }
    })
    .await;
    assert!(disconnected.is_ok(), "sender was not disconnected");

    // ...and the oversized message is never broadcast to other clients
    let nothing = timeout(Duration::from_millis(500), receiver.next()).await;
    assert!(nothing.is_err(), "receiver unexpectedly got a message");
}

#[tokio::test]
async fn handles_many_concurrent_clients() {
    let port = start_bus(25).await;
    let mut sender = connect_and_greet(port).await;

    let mut receivers = Vec::new();
    for _ in 0..10 {
        receivers.push(connect_and_greet(port).await);
    }

    let payload = r#"{"type": "fanout.test", "data": {}}"#;
    sender.send(Message::Text(payload.into())).await.unwrap();

    for receiver in &mut receivers {
        assert_eq!(next_text(receiver).await, payload);
    }
}

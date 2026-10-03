//! A plain-HTTP `GET /health` on the bus port, for orchestrator probes.
//!
//! The bus speaks WebSocket on one port, and a `tcpSocket` probe on it proves
//! only that the port is open (and makes the bus log a handshake that never
//! happens), while a full bus round-trip is heavy. This answers `200 OK`
//! without a WebSocket upgrade, so a probe can use `httpGet` like any other
//! service.
//!
//! The connection is classified by *peeking* at its first bytes, without
//! consuming them, so a WebSocket client's handshake reaches the upgrade code
//! untouched. "Healthy" here means the accept loop is running and the runtime
//! is answering; it deliberately does not look at the broadcast channel, since
//! a probe that fails on a busy bus would restart the bus under load.

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout_at, Instant};

/// How long a connection may take to show whether it is a health probe.
const CLASSIFY_TIMEOUT: Duration = Duration::from_secs(2);
/// The most request header bytes read (and discarded) before answering.
const MAX_REQUEST_BYTES: usize = 8 * 1024;
const REQUEST_HEAD: &[u8] = b"GET /health";
const RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
Content-Length: 3\r\n\
Cache-Control: no-store\r\n\
Connection: close\r\n\
\r\n\
ok\n";

/// What the first bytes of a connection say about it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Start {
    /// `GET /health` followed by a space or a query string.
    Health,
    /// Anything else, including `/healthz` and a WebSocket handshake.
    Other,
    /// Too few bytes to tell yet.
    NeedMore,
}

pub(crate) fn classify(buf: &[u8]) -> Start {
    let common = buf.len().min(REQUEST_HEAD.len());
    if buf[..common] != REQUEST_HEAD[..common] {
        return Start::Other;
    }
    match buf.get(REQUEST_HEAD.len()) {
        None => Start::NeedMore,
        Some(b' ') | Some(b'?') => Start::Health,
        Some(_) => Start::Other,
    }
}

/// Whether this connection is a health probe. Looks at the start of the
/// stream without consuming it; a connection that closes, or that does not
/// say within [`CLASSIFY_TIMEOUT`], is treated as not a probe and left for the
/// WebSocket code to handle as it always has.
pub(crate) async fn is_health_request(stream: &TcpStream) -> bool {
    let deadline = Instant::now() + CLASSIFY_TIMEOUT;
    let mut buf = [0u8; 16];
    loop {
        let n = match timeout_at(deadline, stream.peek(&mut buf)).await {
            Ok(Ok(n)) if n > 0 => n,
            _ => return false,
        };
        match classify(&buf[..n]) {
            Start::Health => return true,
            Start::Other => return false,
            // peek does not consume, so it returns at once with the same
            // bytes until more arrive: wait a moment instead of spinning.
            Start::NeedMore => sleep(Duration::from_millis(2)).await,
        }
    }
}

/// Read and discard the request headers, then answer `200 OK` and close.
/// Reading first matters: closing a socket with unread data resets it, and a
/// probe would see a failure instead of the response.
pub(crate) async fn serve(mut stream: TcpStream) -> std::io::Result<()> {
    let deadline = Instant::now() + CLASSIFY_TIMEOUT;
    let mut seen: Vec<u8> = Vec::with_capacity(512);
    let mut chunk = [0u8; 512];
    while seen.len() < MAX_REQUEST_BYTES && !seen.windows(4).any(|w| w == b"\r\n\r\n") {
        match timeout_at(deadline, stream.read(&mut chunk)).await {
            Ok(Ok(0)) | Err(_) => break,
            Ok(Ok(n)) => seen.extend_from_slice(&chunk[..n]),
            Ok(Err(e)) => return Err(e),
        }
    }
    stream.write_all(RESPONSE).await?;
    stream.shutdown().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_is_recognised_with_a_space_or_a_query() {
        assert_eq!(classify(b"GET /health HTTP/1.1\r\n"), Start::Health);
        assert_eq!(classify(b"GET /health?verbose=1 HTTP/1.1"), Start::Health);
    }

    #[test]
    fn other_paths_and_websocket_handshakes_are_not_health() {
        assert_eq!(classify(b"GET /core HTTP/1.1\r\n"), Start::Other);
        assert_eq!(classify(b"GET /healthz HTTP/1.1\r\n"), Start::Other);
        assert_eq!(classify(b"GET /health/deep HTTP/1.1"), Start::Other);
        assert_eq!(classify(b"POST /health HTTP/1.1"), Start::Other);
        assert_eq!(classify(b"\x16\x03\x01"), Start::Other); // a TLS hello
    }

    #[test]
    fn a_partial_request_waits_for_more_bytes() {
        assert_eq!(classify(b""), Start::NeedMore);
        assert_eq!(classify(b"GET"), Start::NeedMore);
        assert_eq!(classify(b"GET /heal"), Start::NeedMore);
        assert_eq!(classify(b"GET /health"), Start::NeedMore);
        // ...but a prefix that already diverges is decided at once.
        assert_eq!(classify(b"GET /c"), Start::Other);
    }
}

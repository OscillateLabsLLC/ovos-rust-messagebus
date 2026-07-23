# OVOS Rust Messagebus

[![Status: Active](https://img.shields.io/badge/status-active-brightgreen)](https://github.com/OscillateLabsLLC/.github/blob/main/SUPPORT_STATUS.md)

This is a Rust implementation of the OpenVoiceOS (OVOS) messagebus, providing a fast and efficient communication backbone for OVOS components. Think of it as the "nervous system" of the OVOS platform.

## Requirements

- Rust (latest stable version recommended)
- Cargo (comes with Rust)

## Quick Start

Clone the repository and navigate to the project directory:

```sh
git clone https://github.com/OscillateLabsLLC/ovos-rust-messagebus
cd ovos-rust-messagebus
```

### Building and Running

To build and run the project in debug mode:

```sh
cargo build
cargo run
```

For production use, compile with optimizations:

```sh
cargo build --release
```

The optimized `ovos_messagebus` binary will be in the `target/release` directory.

## Configuration

Since there is no Rust port of ovos-utils, configuration is done via environment variables and/or a configuration file.

### Using a Configuration File

The most backwards-compatible option is to set the `OVOS_BUS_CONFIG_FILE` environment variable:

```sh
OVOS_BUS_CONFIG_FILE=/home/ovos/.config/mycroft/mycroft.conf /usr/local/bin/ovos_messagebus
# or
OVOS_BUS_CONFIG_FILE=/home/neon/.config/neon/neon.yaml /usr/local/bin/ovos_messagebus
```

The configuration file should be in YAML or JSON format. Comments (lines starting with `//`) in JSON are supported and will be stripped before parsing. Please note that this is not a full implementation of JSONC.

### Using Environment Variables

Alternatively, you can set the environment variables directly:

```sh
OVOS_BUS_HOST=10.10.10.10 OVOS_BUS_PORT=8181 /usr/local/bin/ovos_messagebus
```

### Available Environment Variables

- `OVOS_BUS_HOST` (default: `127.0.0.1`)
- `OVOS_BUS_PORT` (default: `8181`)
- `OVOS_BUS_CONFIG_FILE` (default: none)
- `OVOS_BUS_MAX_MSG_SIZE` (default: `25`, in MB)
- `OVOS_BUS_ROUTE` (default: `/core`)
- `OVOS_BUS_USE_SSL` (default: `false`) NOTE: If the environment variable exists SSL will be enabled.
- `OVOS_BUS_MSG_BUFFER_CAPACITY` (default: `1024`) — the broadcast channel buffer size. See [Architecture](#architecture) for details.
- `RUST_LOG` (default: unset) — controls log verbosity. Examples: `RUST_LOG=info` for startup and connection events, `RUST_LOG=debug` for connection lifecycle, `RUST_LOG=trace` for per-message logging.

Environment variables take precedence over settings in the configuration file.

### Additional Configuration

Any other settings must be configured in `mycroft.conf` or a similar OVOS-compatible configuration file.

## Architecture

The messagebus uses a `tokio::sync::broadcast` channel for fan-out. When a client sends a message, it's placed into a single shared broadcast channel, and every connected subscriber receives a clone. Because `Utf8Bytes` (tungstenite 0.28) is backed by an `Arc`, cloning a message to N subscribers is N reference count increments, not N string copies.

Outbound writes are batched: the write loop collects up to 64 pending messages (or 256KB, whichever comes first) and flushes them in a single syscall, reducing per-message overhead under load.

### Slow consumer policy

The broadcast buffer has a fixed capacity (default 1024 messages, configurable via `OVOS_BUS_MSG_BUFFER_CAPACITY` or `message_buffer_capacity` in the config file; values below 1 are clamped to 1). If a subscriber falls behind by more than this many messages, it is disconnected rather than allowed to accumulate unbounded backlog.

This is an intentional tradeoff: it caps memory usage and prevents one stalled client from degrading the bus for everyone else, at the cost of dropping that client's connection. For typical OVOS workloads (sequential conversational traffic at 10-50 messages/sec), a client would need to be completely unresponsive for 20-100 seconds before hitting the limit. The default of 1024 is appropriate for most deployments. Increase it if you have a workload with sustained high-throughput bursts where temporary receiver lag is expected and acceptable.

### Connection handling

TCP listen backlog is set to 1024 to handle burst connection scenarios (100+ simultaneous clients). `TCP_NODELAY` is enabled on all accepted sockets to minimize latency. Max message size is enforced at the tungstenite protocol layer — oversized frames are rejected before reaching application code, and the error is logged with the actual size, limit, and the config knob to change it.

### What this doesn't do

**Server-side WebSocket pings.** The server does not send ping frames. Dead connections are detected when the next write fails or when the OS TCP keepalive fires. This means a client that silently disappears (network drop, OOM kill) will hold its broadcast subscription open until the TCP stack notices. In practice this is harmless — the subscription sits idle and gets cleaned up on the next failed write — but it means the server won't proactively notice a dead client. Client-side keepalive (which most WebSocket libraries enable by default) covers this in the other direction.

**Per-message routing or filtering.** Every message is broadcast to every connected client. There is no topic-based subscription or message filtering at the server level. This matches the OVOS bus protocol where all components see all traffic.

## Docker

This project includes a Dockerfile for creating a minimal container with the OVOS Rust Messagebus.

### Building the Docker Image

To build the Docker image, run the following command in the project root:

```sh
docker build -t ovos-rust-messagebus .
```

### Running the Docker Container

To run the container:

```sh
docker run -p 8181:8181 -e OVOS_BUS_HOST=127.0.0.1 ovos-rust-messagebus
```

You can adjust the port mapping and environment variables as needed.

## Development

### Running Tests

To run the test suite:

```sh
cargo test --all-features -- --test-threads=1
```

The suite includes unit tests for config parsing and write batching, plus WebSocket integration tests that exercise a live bus (greeting, broadcast fan-out, message ordering, oversized-message enforcement). The integration tests bind real sockets, so run them with `--test-threads=1` as shown.

### Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, code style, and how to submit pull requests.

## License

Apache-2.0

## Contact

[Open an issue on GitHub](https://github.com/OscillateLabsLLC/ovos-rust-messagebus/issues)

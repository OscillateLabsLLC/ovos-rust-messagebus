# OVOS Rust Messagebus

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

- `OVOS_BUS_HOST` (default: `0.0.0.0`)
- `OVOS_BUS_PORT` (default: `8181`)
- `OVOS_BUS_CONFIG_FILE` (default: none)
- `OVOS_BUS_MAX_MSG_SIZE` (default: `25`, in MB)

Environment variables take precedence over settings in the configuration file.

### Additional Configuration

Any other settings must be configured in `mycroft.conf` or a similar OVOS-compatible configuration file.

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
docker run -p 8181:8181 -e OVOS_BUS_HOST=0.0.0.0 ovos-rust-messagebus
```

You can adjust the port mapping and environment variables as needed.

## Development

### Running Tests

To run the test suite:

```sh
cargo test
```

...except we don't have tests. Please feel free to contribute!

### Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

Apache-2.0

## Contact

mike@oscillatelabs.net

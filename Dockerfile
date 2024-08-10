# Build stage
FROM rust:1.73-slim-bullseye AS builder

LABEL org.opencontainers.image.title="OpenVoiceOS Rust message bus image"
LABEL org.opencontainers.image.description="Message bus service, the nervous system of OpenVoiceOS (Rust implementation)"
LABEL org.opencontainers.image.documentation="https://github.com/OscillateLabsLLC/ovos-rust-messagebus"
LABEL org.opencontainers.image.source="https://github.com/OscillateLabsLLC/ovos-rust-messagebus"
LABEL org.opencontainers.image.vendor="Oscillate Labs, LLC"
LABEL org.opencontainers.image.license="Apache-2.0"

WORKDIR /usr/src/ovos-rust-messagebus

# Copy only the files needed for dependency resolution
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies - this layer will be cached unless Cargo.toml or Cargo.lock change
RUN cargo build --release

# Remove the dummy main.rs
RUN rm src/main.rs

# Now copy the actual source code
COPY src ./src

# Build the application
RUN cargo build --release

# Identify dynamic libraries
RUN ldd /usr/src/ovos-rust-messagebus/target/release/ovos_messagebus | tr -s '[:blank:]' '\n' | grep '^/' | \
    xargs -I % sh -c 'mkdir -p $(dirname deps%); cp % deps%;'

# Final stage
FROM scratch

# Copy the binary
COPY --from=builder /usr/src/ovos-rust-messagebus/target/release/ovos_messagebus /ovos_messagebus

# Copy dynamic libraries
COPY --from=builder /usr/src/ovos-rust-messagebus/deps /

# Copy SSL certificates (required for TLS/SSL operations)
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Set the entry point
ENTRYPOINT ["/ovos_messagebus"]

# Start with a rust alpine image
FROM rust:1-alpine3.19
# This is important, see https://github.com/rust-lang/docker-rust/issues/85
ENV RUSTFLAGS="-C target-feature=-crt-static"
# if needed, add additional dependencies here
RUN apk add --no-cache musl-dev
# set the workdir and copy the source into it
WORKDIR /app
COPY ./ /app
# do a release build
RUN cargo build --release
RUN strip target/release/ovos_messagebus

# use a plain alpine image, the alpine version needs to match the builder
FROM alpine:3.19
LABEL org.opencontainers.image.title="OpenVoiceOS Rust message bus image"
LABEL org.opencontainers.image.description="Message bus service, the nervous system of OpenVoiceOS (Rust implementation)"
LABEL org.opencontainers.image.documentation="https://github.com/OscillateLabsLLC/ovos-rust-messagebus"
LABEL org.opencontainers.image.source="https://github.com/OscillateLabsLLC/ovos-rust-messagebus"
LABEL org.opencontainers.image.vendor="Oscillate Labs, LLC"
LABEL org.opencontainers.image.license="Apache-2.0"
# if needed, install additional dependencies here
RUN apk add --no-cache libgcc
# copy the binary into the final image
COPY --from=0 /app/target/release/ovos_messagebus .
# set the binary as entrypoint
ENTRYPOINT ["/ovos_messagebus"]
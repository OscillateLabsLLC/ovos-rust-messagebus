FROM clux/muslrust:stable as builder
WORKDIR /app
ADD --chown=rust:rust . ./

RUN cargo build --release --bins
RUN pwd && ls -lah /app/target/release
RUN find /app/target -name ovos_messagebus -type f

FROM scratch

COPY --from=builder /app/target/aarch64-unknown-linux-musl/release/ovos_messagebus /app/ovos_messagebus

EXPOSE 8181

CMD ["/app/ovos_messagebus"]

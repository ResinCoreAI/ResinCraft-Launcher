# ResinCraft Relay: one static binary (musl) in a minimal image.
#   docker build -t resincraft-relay relay/
FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock relay.example.toml ./
COPY src ./src
RUN cargo build --release --locked --bin resincraft-relay

# distroless/static: no shell, runs as uid 65532 ("nonroot"). The binary carries its own root certificates.
FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=build /src/target/release/resincraft-relay /usr/local/bin/resincraft-relay
COPY relay.example.toml /usr/share/resincraft-relay/relay.example.toml
EXPOSE 25565 7000 443 19200-19299/udp
ENTRYPOINT ["/usr/local/bin/resincraft-relay", "--config", "/etc/resincraft-relay/relay.toml"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s CMD ["/usr/local/bin/resincraft-relay", "--config", "/etc/resincraft-relay/relay.toml", "health"]

FROM rust:1-slim-bookworm AS build
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY apps/proxy ./apps/proxy
RUN cargo build --release --manifest-path apps/proxy/Cargo.toml

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl libssl3 && rm -rf /var/lib/apt/lists/* && mkdir -p /data
COPY --from=build /app/apps/proxy/target/release/manabus-proxy /usr/local/bin/manabus-proxy
EXPOSE 8080
CMD ["manabus-proxy"]

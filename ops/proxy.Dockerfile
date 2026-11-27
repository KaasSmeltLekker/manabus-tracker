FROM rust:1-slim AS build
WORKDIR /app
COPY apps/proxy ./apps/proxy
RUN cargo build --release --manifest-path apps/proxy/Cargo.toml

FROM debian:stable-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/* && mkdir -p /data
COPY --from=build /app/apps/proxy/target/release/manabus-proxy /usr/local/bin/manabus-proxy
EXPOSE 8080
CMD ["manabus-proxy"]

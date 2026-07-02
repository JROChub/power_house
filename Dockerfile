FROM rust:1.88-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --locked --features net --bin julian

FROM debian:bookworm-slim
ARG POWER_HOUSE_VERSION=0.3.24
LABEL org.opencontainers.image.title="Power House" \
  org.opencontainers.image.version="${POWER_HOUSE_VERSION}" \
  org.opencontainers.image.source="https://github.com/JROChub/power_house"
RUN apt-get update \
  && apt-get install -y ca-certificates \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /data
COPY --from=builder /app/target/release/julian /usr/local/bin/julian
ENTRYPOINT ["/usr/local/bin/julian"]

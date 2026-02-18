FROM rust:1.78 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --features net --bin julian

FROM debian:bookworm-slim
RUN apt-get update \
  && apt-get install -y ca-certificates \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /data
COPY --from=builder /app/target/release/julian /usr/local/bin/julian
ENTRYPOINT ["/usr/local/bin/julian"]

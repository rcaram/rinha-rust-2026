FROM rust:1.87-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml build.rs ./
COPY src ./src
COPY resources ./resources

RUN cargo build --release

FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/rinha-rust /app/rinha-rust
COPY --from=builder /app/resources /app/resources

EXPOSE 3000

ENTRYPOINT ["/app/rinha-rust"]

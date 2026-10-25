# Multi-stage ultra-lightweight Dockerfile for MetaEnricher
# Stage 1: Build binary
FROM rust:1-slim as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN apt-get update && apt-get install -y pkg-config libssl-dev && \
    cargo build --release

# Stage 2: Final minimal runtime image (~60MB)
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    python3 \
    && curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp \
    && chmod a+rx /usr/local/bin/yt-dlp \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/meta_enricher /usr/local/bin/meta_enricher

ENV PORT=8080
EXPOSE 8080

CMD ["meta_enricher"]

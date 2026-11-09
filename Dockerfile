# Multi-stage Dockerfile for Zirc Language
FROM rust:1.94-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy Cargo files first for better caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

RUN cargo build --release --workspace

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1000 zirc

WORKDIR /app

COPY --from=builder /app/target/release/zirc /usr/local/bin/
COPY --from=builder /app/target/release/zirc-repl /usr/local/bin/
COPY --from=builder /app/target/release/zirc-lint /usr/local/bin/
COPY --from=builder /app/target/release/zirc-lsp /usr/local/bin/

COPY examples/ ./examples/
COPY stdlib/ ./stdlib/

ENV ZIRC_PATH=/app/stdlib
RUN chown -R zirc:zirc /app

USER zirc

CMD ["zirc-repl"]

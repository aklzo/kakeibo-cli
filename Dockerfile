# ─── Build stage ─────────────────────────────────────────────────────────────
FROM rust:1-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN cargo build --release --bin kakeibo-api && \
    strip target/release/kakeibo-api

# ─── Runtime stage ────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/kakeibo-api /usr/local/bin/kakeibo-api

EXPOSE 8080

CMD ["kakeibo-api"]

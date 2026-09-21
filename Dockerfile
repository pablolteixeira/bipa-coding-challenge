# --- Build stage -------------------------------------------------------------
FROM rust:1.92-bookworm AS builder
WORKDIR /app

# Build dependencies against placeholder sources first, so this layer is
# cached and code-only changes rebuild in seconds.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && echo 'fn main() {}' > src/main.rs \
    && touch src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY src ./src
COPY migrations ./migrations
# Make sure cargo sees the real sources as newer than the placeholder build.
RUN touch src/main.rs src/lib.rs \
    && cargo build --release --locked

# --- Runtime stage -----------------------------------------------------------
FROM debian:bookworm-slim

# CA certificates are needed for HTTPS calls to mempool.space.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --no-create-home --uid 10001 app

COPY --from=builder /app/target/release/bipa-nodes /usr/local/bin/bipa-nodes

USER app
ENV BIND_ADDR=0.0.0.0:3000
EXPOSE 3000
CMD ["bipa-nodes"]

# syntax=docker/dockerfile:1.7

FROM rust:1-slim-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY templates ./templates
COPY src ./src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked \
    && cp /app/target/release/cpa-portal /tmp/cpa-portal

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /app/data

WORKDIR /app
COPY --from=builder /tmp/cpa-portal /usr/local/bin/cpa-portal

ENV CPA_PORTAL__SERVER__LISTEN=0.0.0.0:8080
EXPOSE 8080
CMD ["cpa-portal"]

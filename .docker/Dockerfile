# Multi-stage Dockerfile to build and export the obsctl binary cleanly

# Stage 1: Builder
FROM rust:1.72-slim-bullseye as builder

RUN apt-get update && apt-get install -y \
  pkg-config \
  libssl-dev \
  build-essential \
  libclang-dev \
  curl \
  ca-certificates && \
  curl -fsSL https://just.systems/install.sh | bash -s -- --to /usr/local/bin && \
  cargo install cargo-audit

WORKDIR /usr/src/app
COPY . .

RUN cargo build --release

# Stage 2: Minimal output image (optional if needed)
FROM debian:bullseye-slim AS exportable
LABEL maintainer="Metro"

COPY --from=builder /usr/src/app/target/release/obsctl /obsctl

ENTRYPOINT ["/obsctl"]

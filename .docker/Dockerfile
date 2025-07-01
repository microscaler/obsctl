FROM rust:1.81-slim-bullseye AS builder

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

# Run checks and tests as part of the build
RUN cargo fmt --all -- --check
RUN cargo clippy --all-targets --all-features -- -D warnings
RUN cargo check
RUN cargo test --all

RUN cargo build --release

# Stage 2: Minimal output image (optional if needed)
FROM debian:bullseye-slim AS exportable
LABEL maintainer="Metro"

COPY --from=builder /usr/src/app/target/release/obsctl /obsctl

ENTRYPOINT ["/obsctl"]

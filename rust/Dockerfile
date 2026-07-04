# Multi-stage build with cargo-chef for cached dependency layers.
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies first — cached until Cargo.toml/lock change.
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin ecommerce

# Minimal runtime; migrations are embedded in the binary via sqlx::migrate!.
FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --no-create-home --user-group app
COPY --from=builder /app/target/release/ecommerce /usr/local/bin/ecommerce
EXPOSE 8080
USER app
HEALTHCHECK --interval=10s --timeout=3s --retries=5 --start-period=20s \
    CMD ["ecommerce", "healthcheck"]
ENTRYPOINT ["ecommerce"]

FROM rust:slim AS builder

WORKDIR /app

# Install cargo-chef for dependency caching
RUN cargo install cargo-chef

# Read recipe
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main! {}" > src/main.rs
RUN cargo chef prepare --recipe-path recipe.json

# Cache dependencies
FROM builder AS chef
WORKDIR /app
COPY --from=builder /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
FROM builder AS build
WORKDIR /app
COPY --from=chef /app/target target
COPY . .
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=build /app/target/release/bricks-api .

EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=5s --retries=3 --start-period=30s \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["./bricks-api"]

# Build stage
FROM rust:1.81-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY src/ src/

# Build with release optimizations
RUN cargo build --release --bin rustai

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/rustai /app/rustai

# Copy default config
COPY rustai.toml /app/rustai.toml

# Create plugins directory
RUN mkdir -p /etc/rustai/plugins

# Health check
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

EXPOSE 8080 9090

ENTRYPOINT ["/app/rustai"]
CMD ["--config", "/app/rustai.toml"]

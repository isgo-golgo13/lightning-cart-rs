# =============================================================================
# Lightning-Cart RS - Multi-stage Docker Build
# =============================================================================
# Build: docker build -t lightning-cart-rs .
# Run:   docker run -p 8080:8080 --env-file .env lightning-cart-rs

# -----------------------------------------------------------------------------
# Stage 1: Build
# -----------------------------------------------------------------------------
FROM rust:1.83-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock* ./
COPY crates ./crates

# Build release binary
RUN cargo build --release -p pay-api

# -----------------------------------------------------------------------------
# Stage 2: Runtime
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/lightning-cart /usr/local/bin/

# Copy config files
COPY config ./config

# Create non-root user
RUN useradd -ms /bin/bash appuser
USER appuser

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Run the binary
ENV HOST=0.0.0.0
ENV PORT=8080
ENV RUST_LOG=info

CMD ["lightning-cart"]

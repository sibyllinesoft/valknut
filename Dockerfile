# Multi-stage Docker build for Valknut
# Optimized for production deployment and CI/CD usage

# Build stage - Use latest Rust with all build tools
FROM rust:1.75-slim-bullseye as builder

# Install system dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user for security
RUN groupadd -r valknut && useradd -r -g valknut valknut

# Set up working directory
WORKDIR /app

# Copy dependency files first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ ./.cargo/

# Copy source code
COPY src/ ./src/
COPY templates/ ./templates/
COPY datasets/ ./datasets/

# Build with release optimizations and all features
RUN cargo build --release --all-features \
    && strip target/release/valknut \
    && chmod +x target/release/valknut

# Runtime stage - Minimal base image
FROM debian:bullseye-slim

# Install minimal runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create app user with same UID/GID as builder
RUN groupadd -r valknut && useradd -r -g valknut valknut

# Create necessary directories
RUN mkdir -p /app/cache /app/reports /app/config \
    && chown -R valknut:valknut /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/valknut /usr/local/bin/valknut
COPY --from=builder --chown=valknut:valknut /app/templates/ /app/templates/

# Set up working directory and user
WORKDIR /app
USER valknut

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD valknut --version || exit 1

# Default configuration
ENV VALKNUT_CACHE_DIR=/app/cache \
    VALKNUT_CONFIG_PATH=/app/config/valknut.toml \
    VALKNUT_LOG_LEVEL=info \
    RUST_LOG=valknut=info

# Expose volume for mounting source code
VOLUME ["/workspace", "/app/cache", "/app/reports", "/app/config"]

# Default entrypoint
ENTRYPOINT ["valknut"]
CMD ["--help"]

# Labels for metadata
LABEL org.opencontainers.image.title="Valknut" \
      org.opencontainers.image.description="High-performance code analysis engine" \
      org.opencontainers.image.version="1.0.0" \
      org.opencontainers.image.vendor="Valknut Project" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.source="https://github.com/your-org/valknut" \
      org.opencontainers.image.documentation="https://valknut.dev/docs"
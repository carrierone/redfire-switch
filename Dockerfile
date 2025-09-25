# Multi-stage Dockerfile for RedFire Anti-Fraud Monitoring
# ECPA-compliant voice monitoring and fraud detection

# Build stage
FROM rust:1.70-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    libsqlite3-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user for security
RUN useradd -r -m -s /bin/false redfire

# Set working directory
WORKDIR /usr/src/app

# Copy dependency manifests
COPY Cargo.toml Cargo.lock ./
COPY redfire-codec-engine/Cargo.toml ./redfire-codec-engine/
COPY redfire-sip-stack/Cargo.toml ./redfire-sip-stack/
COPY redfire-sip-stack-minimal/Cargo.toml ./redfire-sip-stack-minimal/
COPY redfire-mcp-server/Cargo.toml ./redfire-mcp-server/

# Copy source code
COPY . .

# Build the application
RUN cargo build --release --features="services,database" --bin redfire-cli

# Anti-fraud service target
FROM debian:bullseye-slim as antifraud

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    libpq5 \
    libsqlite3-0 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -m -s /bin/false redfire

# Create necessary directories
RUN mkdir -p /var/lib/redfire/recordings \
    /dev/shm/redfire-recordings \
    /var/log/redfire \
    && chown -R redfire:redfire /var/lib/redfire /var/log/redfire

# Copy binary from builder
COPY --from=builder /usr/src/app/target/release/redfire-cli /usr/local/bin/

# Copy configuration templates
COPY config/ /etc/redfire/

# Security hardening
RUN chmod 755 /usr/local/bin/redfire-cli

# Health check script
RUN echo '#!/bin/bash\ncurl -f http://localhost:8080/health || exit 1' > /usr/local/bin/health-check.sh && \
    chmod +x /usr/local/bin/health-check.sh

# Environment variables
ENV RUST_LOG=info \
    DATABASE_URL=postgresql://redfire:password@localhost:5432/redfire_antifraud \
    MONITORING_ENABLED=true \
    ECPA_COMPLIANCE_MODE=fraud_prevention

# Switch to non-root user
USER redfire

# Expose ports
EXPOSE 8080 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD /usr/local/bin/health-check.sh

# Start anti-fraud monitoring service
CMD ["/usr/local/bin/redfire-cli", "server", "--service", "anti-fraud", "--port", "8080"]

# Batch worker target
FROM antifraud as worker

# Switch to root to modify startup
USER root

# Worker-specific environment
ENV WORKER_TYPE=batch_processor \
    BATCH_INTERVAL_MINUTES=2 \
    PROCESSING_THREADS=4

# Remove health check for worker
HEALTHCHECK NONE

# Switch back to app user
USER redfire

# Start batch processing worker
CMD ["/usr/local/bin/redfire-cli", "worker", "--type", "batch_processor"]

# Development target
FROM builder as development

# Install development tools
RUN apt-get update && apt-get install -y \
    gdb \
    strace \
    valgrind \
    && rm -rf /var/lib/apt/lists/*

# Don't change user for development
WORKDIR /usr/src/app

# Development command
CMD ["cargo", "run", "--bin", "redfire-cli"]

# Production target (default)
FROM antifraud as production

# Production optimizations
ENV RUST_LOG=warn

# Final production command
CMD ["/usr/local/bin/redfire-cli", "server", "--service", "anti-fraud", "--port", "8080", "--production"]
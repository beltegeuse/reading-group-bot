# Multi-stage build for lightweight Docker image
# Stage 1: Builder
FROM rust:alpine AS builder

# Install dependencies required for building
RUN apk add --no-cache \
    musl-dev \
    sqlite-dev \
    openssl-dev \
    ca-certificates \
    build-base

# Set working directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock* ./
COPY diesel.toml ./

# Copy source tree
COPY src ./src
COPY migrations ./migrations
COPY templates ./templates

# Build the application
ENV RUSTFLAGS="-C target-feature=-crt-static"
RUN cargo build --release

# Stage 2: Runtime
FROM alpine:latest

# Install runtime dependencies
RUN apk add --no-cache \
    sqlite-libs \
    openssl \
    ca-certificates \
    libgcc

# Create app directory
WORKDIR /app

# Copy the compiled binary from builder
COPY --from=builder /app/target/release/reading-group-bot ./

# Copy static files and templates
COPY static ./static
COPY templates ./templates
COPY migrations ./migrations
COPY Rocket.toml ./
COPY docker-entrypoint.sh ./

# Create directories for database and uploads
RUN chmod +x /app/docker-entrypoint.sh && mkdir -p /app/data /app/static/pdfs /app/static/thumbnails

# Expose the port that Rocket runs on
EXPOSE 3001

# Set environment variables
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PROFILE=release
ENV ROCKET_PORT=3001
ENV ROCKET_DATABASES={sqlite_database={url="/app/data/db.sqlite"}}

# Create a volume for persistent database storage
VOLUME ["/app/data", "/app/static/pdfs", "/app/static/thumbnails"]

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD ["/bin/sh", "-c", "[ -f /app/data/db.sqlite ] || exit 0"]

# Run the application
CMD ["/app/docker-entrypoint.sh"]

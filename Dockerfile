# Build stage - use Debian with rustup for latest Rust (v2)
FROM debian:bookworm-slim AS builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    curl \
    pkg-config \
    libssl-dev \
    ca-certificates \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Install rustup and latest stable Rust (1.94+)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

# Install wasm target and trunk
RUN rustup target add wasm32-unknown-unknown
RUN cargo install trunk --locked
RUN cargo install wasm-bindgen-cli --locked

# Set working directory
WORKDIR /app/frontend-rust

# Copy only the frontend-rust directory
COPY frontend-rust/ .

# Set env var to bypass git requirement in spacetimedb-lib build.rs
ENV SPACETIMEDB_NIX_BUILD_GIT_COMMIT="docker-build"

# Clean pre-built worker binaries to force rebuild from source
RUN rm -rf pkg/worker/*.wasm pkg/worker/*.js

# Build the frontend
RUN trunk build --release

# Production stage - serve with nginx
FROM nginx:alpine

# Copy built files
COPY --from=builder /app/frontend-rust/dist /usr/share/nginx/html

# Copy nginx config for SPA routing
RUN echo 'server { \
    listen 80; \
    root /usr/share/nginx/html; \
    index index.html; \
    location / { \
        try_files $uri $uri/ /index.html; \
    } \
}' > /etc/nginx/conf.d/default.conf

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]

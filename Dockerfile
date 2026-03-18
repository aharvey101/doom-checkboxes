# Build stage - use a Rust image with wasm target
FROM rust:1.83-slim-bookworm AS builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    curl \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install wasm target and trunk
RUN rustup target add wasm32-unknown-unknown
RUN cargo install trunk --locked

# Set working directory
WORKDIR /app/frontend-rust

# Copy only the frontend-rust directory
COPY frontend-rust/ .

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

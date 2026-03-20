#!/bin/bash
set -e

echo "Building worker WASM..."

# Build worker binary
cargo build --bin worker --target wasm32-unknown-unknown --release

# Run wasm-bindgen
wasm-bindgen \
    target/wasm32-unknown-unknown/release/worker.wasm \
    --out-dir pkg/worker \
    --target web \
    --no-typescript

echo "Worker built successfully at pkg/worker/worker.js"

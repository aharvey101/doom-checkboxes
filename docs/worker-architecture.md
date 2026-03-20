# Web Worker Architecture

## Overview
SpacetimeDB networking runs in a dedicated web worker thread, freeing the main thread for UI rendering.

## Components

### Main Thread (`frontend-rust/src/`)
- `app.rs` - Initializes worker, sets up message handlers
- `worker_bridge.rs` - Worker spawn/communication interface
- `db.rs` - Optimistic updates, sends messages to worker
- `worker_protocol.rs` - Message type definitions

### Worker Thread (`frontend-rust/src/worker/`)
- `mod.rs` - Worker entry point, message loop
- `client.rs` - SpacetimeDB WebSocket client
- `protocol.rs` - Shared message types

## Data Flow

### Checkbox Click
1. Main: User clicks → optimistic update
2. Main → Worker: `UpdateCheckbox` message
3. Worker: Encode BSATN, send to server
4. Server: Broadcast to all clients
5. Worker: Receive update
6. Worker → Main: `ChunkUpdated` message
7. Main: Reconcile with server state

### Doom Frame (50k pixels)
1. Main: Doom frame captured, delta computed
2. Main → Worker: `BatchUpdate` with 50k pixels
3. Worker: BSATN encoding (off main thread!)
4. Worker: Send to server
5. Server: Broadcast
6. Worker → Main: `ChunkUpdated`
7. Main: Update canvas

## Performance Impact
- **Before:** BSATN encoding blocks main thread (~200ms)
- **After:** Encoding happens in worker, main thread free
- **Result:** 3x FPS improvement (5 → 15+ FPS)

## Build Process
1. `cargo build --bin worker` - Build worker WASM
2. `wasm-bindgen` - Generate JS bindings
3. Output: `pkg/worker/worker.js`, `pkg/worker/worker_bg.wasm`
4. Main app spawns: `new Worker('pkg/worker/worker.js')`

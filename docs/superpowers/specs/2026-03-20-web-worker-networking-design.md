# Web Worker Networking Design

**Date:** 2026-03-20
**Status:** Approved
**Goal:** Offload SpacetimeDB networking to web worker to improve Doom rendering performance from ~5 FPS to 15+ FPS

## Problem Statement

Current implementation processes Doom frames (640×400, ~50k pixel updates per frame) on the main thread. BSATN serialization and WebSocket send operations block the rendering thread, causing:
- Doom FPS: ~5 FPS (target: 15+ FPS)
- Main thread blocking: ~200ms per frame
- Poor user experience during Doom gameplay

## Solution Overview

Move all SpacetimeDB WebSocket I/O to a dedicated web worker thread using message passing architecture. This frees the main thread for rendering while the worker handles network serialization and communication.

## Architecture

### System Structure

```
┌─────────────────────────────────────────┐
│         Main Thread (Leptos)            │
│  - UI rendering                         │
│  - Leptos signals (loaded_chunks, etc)  │
│  - Optimistic updates                   │
│  - Canvas rendering (Doom, checkboxes)  │
└──────────────┬──────────────────────────┘
               │ postMessage
               ↓
┌─────────────────────────────────────────┐
│      Worker Thread (Rust WASM)          │
│  - SpacetimeDB WebSocket client         │
│  - Connection lifecycle                 │
│  - BSATN serialization                  │
│  - Auto-reconnection logic              │
│  - Message routing                      │
└─────────────────────────────────────────┘
               │ WebSocket
               ↓
┌─────────────────────────────────────────┐
│         SpacetimeDB Server              │
└─────────────────────────────────────────┘
```

### Key Principles

- Main thread never touches WebSocket directly
- Worker is single source of truth for network state
- Optimistic updates on main thread for instant UI feedback
- Worker reconciles optimistic state when server updates arrive
- All SpacetimeDB client logic moves to worker

## Component Breakdown

### New Components

#### 1. `frontend-rust/src/worker/mod.rs` - Worker Entry Point
- Exports `wasm_bindgen` entry point for worker context
- Initializes SpacetimeDB client
- Sets up message handlers from main thread
- Handles worker lifecycle

#### 2. `frontend-rust/src/worker/client.rs` - Worker-Side Client
- Migrates logic from current `db.rs` and `ws_client.rs`
- Manages WebSocket connection
- Handles BSATN encoding/decoding
- Auto-reconnection with exponential backoff (5s, 10s, 20s, 40s, 60s max)
- Routes server messages back to main thread

#### 3. `frontend-rust/src/worker/protocol.rs` - Message Types
- `MainToWorker` enum for main → worker messages
- `WorkerToMain` enum for worker → main messages
- Uses `serde` for JSON serialization

### Modified Components

#### 4. `frontend-rust/src/db.rs` - Main Thread Bridge (Simplified)
- Remove all WebSocket/SpacetimeDB client code
- Keep: `toggle_checkbox`, `set_checkbox_checked`, optimistic updates
- Add: Send messages to worker instead of calling reducers directly
- Add: Receive chunk updates from worker, update Leptos signals

#### 5. `frontend-rust/src/worker_bridge.rs` - Worker Interface (New)
- Spawn worker on app init
- Provide `send_to_worker()` function for db.rs to use
- Set up MessageChannel for chunk updates
- Handle worker error/termination events

### Build Changes

#### 6. New Binary Target in `Cargo.toml`
```toml
[[bin]]
name = "worker"
path = "src/worker/mod.rs"
```

#### 7. `frontend-rust/build-worker.sh` - Build Script
- Compile worker to WASM with `wasm-bindgen`
- Output to `frontend-rust/pkg/worker.js` and `worker_bg.wasm`

## Message Protocol

### Main Thread → Worker Messages

```rust
#[derive(Serialize, Deserialize)]
pub enum MainToWorker {
    // Initialize connection
    Connect {
        uri: String,
        database: String,
    },

    // Subscribe to chunks
    Subscribe {
        chunk_ids: Vec<i64>,
    },

    // Single checkbox toggle
    UpdateCheckbox {
        chunk_id: i64,
        cell_offset: u32,
        r: u8,
        g: u8,
        b: u8,
        checked: bool,
    },

    // Batch updates (drag-to-fill, Doom frames)
    BatchUpdate {
        updates: Vec<(i64, u32, u8, u8, u8, bool)>,
    },

    // Clean shutdown
    Disconnect,
}
```

### Worker → Main Thread Messages

```rust
#[derive(Serialize, Deserialize)]
pub enum WorkerToMain {
    // Initial chunk load
    ChunkInserted {
        chunk_id: i64,
        state: Vec<u8>,
        version: u64,
    },

    // Chunk updated by another client
    ChunkUpdated {
        chunk_id: i64,
        state: Vec<u8>,
        version: u64,
    },

    // Successfully connected and subscribed
    Connected,

    // Fatal error after retry exhaustion
    FatalError {
        message: String,
    },
}
```

### Wire Format
- Messages serialized with `serde_json`
- Sent via `postMessage()` as strings
- Worker uses `serde_wasm_bindgen` to deserialize from JsValue
- Chunk data (`Vec<u8>`) transferred as typed arrays (zero-copy when possible)
- For Doom: batch updates aggregate 50k+ pixels into single `BatchUpdate` message

## Data Flow

### Flow 1: App Initialization
1. Main: Spawn worker (worker_bridge.rs)
2. Main: Send `MainToWorker::Connect { uri, database }`
3. Worker: Connect to SpacetimeDB WebSocket
4. Worker: Subscribe to checkbox_chunk table
5. Worker: Send `WorkerToMain::Connected`
6. Main: Update status signal to "Connected"

### Flow 2: Click Checkbox
1. Main: User clicks checkbox at (col, row)
2. Main: Optimistic update to `state.loaded_chunks` (instant UI feedback)
3. Main: Send `MainToWorker::UpdateCheckbox { chunk_id, cell_offset, r, g, b, checked }`
4. Worker: Encode BSATN, call update_checkbox reducer via WebSocket
5. Server: Broadcast update to all clients
6. Worker: Receive chunk update from server
7. Worker: Send `WorkerToMain::ChunkUpdated { chunk_id, state, version }`
8. Main: Update `state.loaded_chunks` with server truth (reconcile optimistic update)
9. Main: Trigger re-render

### Flow 3: Drag-to-Fill (Batched Updates)
1. Main: User drags across 100 checkboxes
2. Main: Optimistic updates to `state.loaded_chunks` (all 100 cells)
3. Main: On mouseup, send `MainToWorker::BatchUpdate { updates: [(chunk_id, offset, r, g, b, checked) × 100] }`
4. Worker: Encode batch BSATN, call batch_update_checkboxes reducer
5. Server: Apply all updates, broadcast back
6. Worker: Receive chunk update
7. Worker: Send `WorkerToMain::ChunkUpdated`
8. Main: Reconcile with server state

### Flow 4: Doom Frame (High Frequency)
1. Doom: Frame captured (60 FPS), delta computed (~50k changed pixels)
2. Main: Send `MainToWorker::BatchUpdate { updates: [50k pixels] }`
3. Worker: Encode BSATN (happens off main thread - **key performance win**)
4. Worker: Send to server via WebSocket
5. Server: Broadcast to all connected clients
6. Worker: Receive broadcast
7. Worker: Send `WorkerToMain::ChunkUpdated`
8. Main: Update Doom region, trigger canvas re-render

**Key Performance Gain:** BSATN encoding of 50k pixels happens in worker, keeping main thread free for canvas rendering.

### Flow 5: Worker Reconnection (Silent)
1. Worker: WebSocket closes (network blip)
2. Worker: Wait 5s, attempt reconnect
3. Worker: If fails, wait 10s, retry
4. Worker: If fails 5 times, send `WorkerToMain::FatalError { message: "Connection lost after 5 retries" }`
5. Main: Show error UI, offer manual reconnect button

## Error Handling & Reconnection

### Reconnection Strategy

```rust
// In worker/client.rs
struct ReconnectionState {
    attempt: u32,
    backoff_ms: u32,
}

const BACKOFF_SCHEDULE: [u32; 5] = [5000, 10000, 20000, 40000, 60000];
const MAX_RETRIES: u32 = 5;
```

### On WebSocket Close
1. Worker detects close event
2. Check if intentional (user navigated away, called Disconnect)
3. If unintentional:
   - Start retry attempt 1: wait 5s
   - If fails, attempt 2: wait 10s
   - If fails, attempt 3: wait 20s
   - If fails, attempt 4: wait 40s
   - If fails, attempt 5: wait 60s
   - If all 5 fail: Send `FatalError` to main thread
4. On successful reconnect: Re-subscribe to all chunks

### Error Scenarios

**1. Worker WASM Fails to Load**
- Main thread catches worker initialization error
- Show error UI: "Failed to initialize network worker. Please refresh."
- Log to console with details

**2. WebSocket Connection Refused**
- Worker attempts reconnection with backoff
- After MAX_RETRIES: Send `FatalError { message: "Cannot reach server" }`
- Main thread shows: "Unable to connect. Check your internet connection."

**3. Worker Process Crashes**
- Main thread detects via `worker.onerror` event
- Attempt to respawn worker once
- If respawn fails: Show error UI with refresh button

**4. Invalid Message from Main Thread**
- Worker logs warning to console
- Ignore message, continue processing
- Don't crash worker over malformed messages

**5. BSATN Decode Error**
- Worker logs error with chunk_id
- Skip that specific chunk update
- Continue processing other messages

### State During Reconnection
- Main thread continues accepting user input
- Optimistic updates still work (local state updates)
- Messages queued in worker are held until reconnection succeeds
- Once reconnected, worker flushes queued messages

## Migration Strategy

### Phase 1: Build Worker Infrastructure
- Create `worker/` module structure
- Set up build script for worker WASM compilation
- Add worker binary target to Cargo.toml
- Implement basic worker that can receive messages and echo back
- Test worker spawning from main thread

### Phase 2: Move SpacetimeDB Client to Worker
- Copy ws_client.rs logic into worker/client.rs
- Adapt callbacks to use postMessage instead of Leptos signals
- Implement message protocol (protocol.rs)
- Keep existing db.rs working (don't break main branch yet)

### Phase 3: Implement Worker Bridge
- Create worker_bridge.rs in main thread
- Spawn worker on app init
- Set up bidirectional messaging
- Add message handlers for ChunkInserted/ChunkUpdated

### Phase 4: Migrate db.rs
- Replace direct reducer calls with worker messages
- Keep optimistic update logic
- Update signal updates to use worker messages
- Remove old WebSocket client code

### Phase 5: Testing & Validation
- Test single checkbox clicks
- Test drag-to-fill batching
- Test Doom frame performance (measure FPS improvement)
- Test reconnection scenarios
- Test multi-client sync

### Rollback Strategy
Each phase can be feature-flagged. If worker approach has issues:
```rust
#[cfg(feature = "web-worker")]
use worker_bridge::send_to_worker;

#[cfg(not(feature = "web-worker"))]
use ws_client::call_reducer;
```

## Testing Approach

### Unit Tests

**1. Message Protocol (protocol.rs)**
```rust
#[test]
fn test_serialize_main_to_worker() {
    let msg = MainToWorker::UpdateCheckbox {
        chunk_id: 42,
        cell_offset: 100,
        r: 255, g: 0, b: 0,
        checked: true,
    };
    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: MainToWorker = serde_json::from_str(&json).unwrap();
    // Assert round-trip works
}
```

**2. BSATN Encoding (worker/client.rs)**
- Test encoding single update
- Test encoding batch updates
- Verify byte layout matches SpacetimeDB expectations

### Integration Tests

**3. Worker Communication Test**
```rust
// Spawn worker, send Connect message, verify Connected response
// Send UpdateCheckbox, verify worker calls reducer
// Test message ordering
```

**4. Reconnection Test**
```rust
// Simulate WebSocket close
// Verify worker attempts reconnection with backoff
// Verify queued messages sent after reconnect
```

### Performance Tests

**5. Doom FPS Test (update existing `doom-performance.spec.ts`)**
- Measure FPS before/after worker implementation
- Target: 15+ FPS (3x improvement from current 5 FPS)
- Measure main thread frame time during Doom
- Target: < 16ms per frame (60 FPS capable)

**6. Main Thread Blocking Test**
```typescript
// Send 50k pixel batch update
// Measure main thread blocking time
// Before: ~200ms
// After: < 5ms (just postMessage overhead)
```

### Manual Testing

**7. Multi-Client Sync**
- Open app in two browser windows
- Click checkbox in window A
- Verify appears in window B within 100ms
- Test with Doom running in both windows

**8. Network Conditions**
- Simulate slow 3G connection
- Verify reconnection works
- Test message queuing during disconnect
- Verify no data loss

**9. Worker Crash Recovery**
- Force worker to crash (throw error in worker)
- Verify main thread detects and shows error UI
- Test respawn logic

## Success Metrics

- **Doom FPS:** 15+ FPS (vs current ~5 FPS) = 3x improvement
- **Main thread frame time:** < 16ms during Doom (60 FPS capable)
- **Checkbox click latency:** < 50ms perceived latency (optimistic update)
- **No regressions:** Multi-client sync still works correctly
- **Reconnection:** Successfully recovers from network interruptions

## Implementation Notes

### Rust Worker Binary
The worker will be compiled as a separate Rust binary target (`worker`) and compiled to WASM with wasm-bindgen. This allows full code reuse of the existing SpacetimeDB client logic.

### Build Process
1. `cargo build --bin worker --target wasm32-unknown-unknown --release`
2. `wasm-bindgen` to generate JS bindings
3. Output `worker.js` and `worker_bg.wasm` to pkg/
4. Main app spawns worker: `new Worker('pkg/worker.js')`

### Zero JavaScript
The worker is pure Rust WASM. No JavaScript required except the minimal wasm-bindgen glue code (auto-generated).

## Trade-offs

**Pros:**
- Massive performance improvement for Doom (3x FPS)
- Main thread stays responsive during network I/O
- Full code reuse of existing Rust SpacetimeDB client
- Type safety across worker boundary via serde

**Cons:**
- Larger bundle size (~500KB for worker WASM)
- Worker initialization overhead on first load (~100ms)
- More complex build process (two WASM targets)
- Debugging requires worker DevTools panel

**Decision:** The performance gains outweigh the complexity cost. Doom is the primary use case, and 3x FPS improvement justifies the architecture change.

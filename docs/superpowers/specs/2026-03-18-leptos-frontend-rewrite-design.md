# Leptos Frontend Rewrite Design

**Date:** 2026-03-18  
**Status:** Approved  
**Scope:** Rewrite TypeScript frontend in Rust using Leptos framework

## Summary

Port the existing TypeScript "1 Million Checkboxes" frontend to Rust/Leptos with identical functionality: a 1000x1000 grid of checkboxes rendered on canvas, real-time sync via SpacetimeDB, pan/zoom controls, and optimistic updates.

## Architecture

**Approach:** Leptos Signals + Canvas

- Leptos reactive signals manage application state
- Canvas element handles rendering (same as TS version)
- SpacetimeDB Rust SDK with generated bindings for real-time sync
- Pure Leptos (no external HTML file) - styles embedded via `include_str!`

## Project Structure

```
frontend-rust/
├── Cargo.toml           # Dependencies (leptos, spacetimedb-sdk, web-sys)
├── Trunk.toml           # Trunk build configuration
├── index.html           # Minimal HTML shell for trunk
├── src/
│   ├── main.rs          # Entry point, mounts App
│   ├── app.rs           # Root App component
│   ├── components/
│   │   ├── mod.rs
│   │   ├── header.rs    # Status bar + stats display
│   │   └── canvas.rs    # Canvas grid component
│   ├── state.rs         # Reactive state (signals)
│   ├── db.rs            # SpacetimeDB connection + callbacks
│   ├── utils.rs         # Bit manipulation helpers
│   └── styles.css       # Embedded styles
└── generated/           # spacetime generate --lang rust output
```

## State Management

```rust
pub struct AppState {
    // Connection status
    pub status: RwSignal<ConnectionStatus>,
    pub status_message: RwSignal<String>,
    
    // Checkbox data: 125KB for 1M bits
    pub chunk_data: RwSignal<Vec<u8>>,
    
    // Derived count
    pub checked_count: RwSignal<u32>,
    
    // Viewport state
    pub offset_x: RwSignal<f64>,
    pub offset_y: RwSignal<f64>,
    pub scale: RwSignal<f64>,
}

pub enum ConnectionStatus {
    Connecting,
    Connected,
    Error,
}
```

**Reactivity flow:**
1. SpacetimeDB table callbacks update `chunk_data` signal
2. `create_effect` watching `chunk_data` triggers canvas re-render
3. Pan/zoom events update viewport signals, also triggering re-render

**Initial state:**
- `chunk_data`: Empty `Vec<u8>` (125,000 zeros) - renders as all unchecked
- `checked_count`: 0
- `scale`: 1.0
- `offset_x`, `offset_y`: 0.0
- `status`: `Connecting`

## Optimistic Updates

When a user clicks a checkbox:

1. **Immediate local update:** Modify `chunk_data` signal directly, toggle the bit, update `checked_count`
2. **Send to server:** Call `conn.reducers.update_checkbox(0, bit_index, new_value)`
3. **Server broadcast:** Server updates its state and broadcasts to all clients
4. **Reconciliation:** When `on_update` callback fires, it overwrites `chunk_data` with server state

**No conflict resolution needed:** The server is authoritative. If the local optimistic state differs from server state (e.g., another client toggled the same checkbox), the server's `on_update` will correct it. This matches the TS behavior exactly.

**Clicks during disconnect:** If `status` is `Error`, clicks still update local state optimistically. When reconnected, the subscription will receive the current server state, overwriting local changes. This is acceptable - same behavior as TS version.

## Components

### App (app.rs)

Root component that:
- Creates `AppState` with default values
- Spawns async connection to SpacetimeDB on mount
- Renders Header and CheckboxCanvas
- Embeds CSS via `include_str!`

### Header (components/header.rs)

Displays:
- Title: "1 Million Checkboxes"
- Connection status badge (connecting/connected/error)
- Stats: checked count, zoom level, control hints

Reactive updates via signal getters in closures.

### CheckboxCanvas (components/canvas.rs)

Core rendering component:
- `NodeRef<Canvas>` for direct canvas access
- `create_effect` re-renders when signals change
- Event handlers:
  - `click`: Toggle checkbox at cursor position
  - `mousedown/move/up`: Shift+drag panning
  - `wheel`: Zoom toward cursor position

Rendering algorithm (same as TS):
1. Clear canvas with grid background color
2. Calculate visible cell range from viewport
3. Draw each visible cell as colored rectangle

## SpacetimeDB Integration

### Environment Detection (db.rs)

```rust
fn is_local() -> bool {
    let window = web_sys::window().expect("no window");
    let hostname = window.location().hostname().unwrap_or_default();
    hostname == "localhost" || hostname == "127.0.0.1"
}
```

### Connection (db.rs)

```rust
pub async fn connect(state: AppState) -> DbConnection {
    let uri = if is_local() {
        "ws://127.0.0.1:3000"
    } else {
        "wss://maincloud.spacetimedb.com"
    };
    
    DbConnection::builder()
        .with_uri(uri)
        .with_database_name("checkboxes")
        .on_connect(/* register callbacks, subscribe */)
        .on_disconnect(move || {
            state.status.set(ConnectionStatus::Error);
            state.status_message.set("Disconnected".to_string());
        })
        .on_connect_error(move |error| {
            state.status.set(ConnectionStatus::Error);
            state.status_message.set(format!("Connection failed: {}", error));
        })
        .build()
        .await
}
```

### Error Handling & Reconnection

**Connection errors:** Set `status` to `Error` with descriptive message. No automatic reconnection - matches TS behavior (user refreshes page to reconnect).

**Disconnect:** Same as connection error - display "Disconnected" status.

**Subscription errors:** Log to console, set `status` to `Error`.

### Table Callbacks

```rust
conn.db.checkbox_chunk.on_insert(|row| {
    state.chunk_data.set(row.state.clone());
    state.checked_count.set(count_bits(&row.state));
});

conn.db.checkbox_chunk.on_update(|_old, new| {
    state.chunk_data.set(new.state.clone());
    state.checked_count.set(count_bits(&new.state));
});
```

### Generated Bindings

Run `spacetime generate --lang rust --out-dir frontend-rust/generated` to generate Rust types from the backend module.

**Expected CheckboxChunk table schema:**

```rust
pub struct CheckboxChunk {
    pub chunk_id: u32,      // Primary key, always 0 for now
    pub state: Vec<u8>,     // 125,000 bytes (1M bits)
    pub version: u64,       // Incremented on each update
}
```

**Expected update_checkbox reducer signature:**

```rust
pub fn update_checkbox(chunk_id: u32, bit_offset: u32, checked: bool);
```

These match the backend `lib.rs` definitions.

## Utilities (utils.rs)

Bit manipulation functions (direct port from TS):

```rust
pub fn get_bit(data: &[u8], bit_index: u32) -> bool;
pub fn set_bit(data: &mut [u8], bit_index: u32, value: bool);
pub fn count_bits(data: &[u8]) -> u32;
```

Grid coordinate conversion:

```rust
pub fn canvas_to_grid(
    mouse_x: f64, mouse_y: f64,
    offset_x: f64, offset_y: f64,
    scale: f64
) -> Option<(u32, u32)>;
```

## Constants

```rust
const GRID_WIDTH: u32 = 1000;
const GRID_HEIGHT: u32 = 1000;
const TOTAL_CHECKBOXES: u32 = 1_000_000;
const CELL_SIZE: f64 = 4.0;

const COLOR_CHECKED: &str = "#2ecc71";
const COLOR_UNCHECKED: &str = "#2c3e50";
const COLOR_GRID: &str = "#1a1a2e";
```

## Build & Run

### Dependencies (Cargo.toml)

```toml
[dependencies]
leptos = { version = "0.7", features = ["csr"] }
spacetimedb-sdk = "2.0"
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
web-sys = { version = "0.3", features = [
    "Window", "Document", "HtmlCanvasElement",
    "CanvasRenderingContext2d", "MouseEvent", "WheelEvent",
    "Element", "DomRect", "console"
] }
js-sys = "0.3"
console_error_panic_hook = "0.1"
```

### Build Commands

```bash
# Generate Rust bindings
spacetime generate --lang rust --out-dir frontend-rust/generated \
    --bin-path backend/target/wasm32-unknown-unknown/release/backend.wasm

# Build and serve (using trunk)
cd frontend-rust && trunk serve
```

## Success Criteria

1. Visual parity with TS version (same colors, layout, grid)
2. Click to toggle checkboxes works
3. Shift+drag panning works
4. Scroll wheel zoom works
5. Real-time sync between multiple browser tabs
6. Optimistic updates provide instant feedback
7. Connection status displays correctly
8. Stats (checked count, zoom level) update reactively

## Out of Scope

- Performance optimizations beyond TS version
- New features not in TS version
- Mobile/touch support (TS version doesn't have it)
- Tests (can be added later)

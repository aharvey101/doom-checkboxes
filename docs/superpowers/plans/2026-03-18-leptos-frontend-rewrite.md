# Leptos Frontend Rewrite Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite the TypeScript checkbox grid frontend in Rust/Leptos with identical functionality.

**Architecture:** Leptos reactive signals manage state; canvas element renders 1M checkboxes; SpacetimeDB Rust SDK handles real-time sync. Pure Leptos approach with embedded CSS.

**Tech Stack:** Rust, Leptos 0.7, SpacetimeDB SDK 2.0, web-sys, wasm-bindgen, Trunk

**Spec:** `docs/superpowers/specs/2026-03-18-leptos-frontend-rewrite-design.md`

---

## File Structure

```
frontend-rust/
├── Cargo.toml           # Update dependencies
├── Trunk.toml           # Create: Trunk build config
├── index.html           # Create: Minimal HTML shell
├── src/
│   ├── main.rs          # Update: Mount App component
│   ├── lib.rs           # Create: Module declarations
│   ├── app.rs           # Create: Root App component
│   ├── state.rs         # Create: AppState + ConnectionStatus
│   ├── constants.rs     # Create: Grid/color constants
│   ├── utils.rs         # Create: Bit manipulation + grid conversion
│   ├── db.rs            # Create: SpacetimeDB connection
│   ├── styles.css       # Create: Embedded styles
│   └── components/
│       ├── mod.rs       # Create: Component exports
│       ├── header.rs    # Create: Header component
│       └── canvas.rs    # Create: Canvas grid component
└── generated/           # spacetime generate output
```

---

## Chunk 1: Project Setup & Core Infrastructure

### Task 1: Update Cargo.toml with dependencies

**Files:**
- Modify: `frontend-rust/Cargo.toml`

- [ ] **Step 1: Update Cargo.toml with all required dependencies**

```toml
[package]
name = "checkbox-frontend"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
leptos = { version = "0.7", features = ["csr"] }
spacetimedb-sdk = "2.0"
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
web-sys = { version = "0.3", features = [
    "Window",
    "Document",
    "HtmlCanvasElement",
    "CanvasRenderingContext2d",
    "MouseEvent",
    "WheelEvent",
    "Element",
    "DomRect",
    "Location",
    "console",
] }
js-sys = "0.3"
console_error_panic_hook = "0.1"
log = "0.4"

[profile.release]
lto = true
opt-level = 'z'
codegen-units = 1
```

- [ ] **Step 2: Verify Cargo.toml is valid**

Run: `cd frontend-rust && cargo check 2>&1 | head -20`
Expected: May show errors about missing lib.rs/main.rs - that's fine, we'll create them next.

- [ ] **Step 3: Commit**

```bash
git add frontend-rust/Cargo.toml
git commit -m "chore(frontend-rust): update Cargo.toml with Leptos dependencies"
```

---

### Task 2: Create Trunk configuration and HTML shell

**Files:**
- Create: `frontend-rust/Trunk.toml`
- Create: `frontend-rust/index.html`

- [ ] **Step 1: Create Trunk.toml**

```toml
[build]
target = "index.html"
dist = "dist"

[watch]
watch = ["src", "index.html"]

[serve]
address = "127.0.0.1"
port = 8080
open = false
```

- [ ] **Step 2: Create minimal index.html for Trunk**

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>1 Million Checkboxes</title>
    <link data-trunk rel="rust" data-wasm-opt="z" />
</head>
<body>
    <div id="app"></div>
</body>
</html>
```

- [ ] **Step 3: Commit**

```bash
git add frontend-rust/Trunk.toml frontend-rust/index.html
git commit -m "chore(frontend-rust): add Trunk config and HTML shell"
```

---

### Task 3: Create constants module

**Files:**
- Create: `frontend-rust/src/constants.rs`

- [ ] **Step 1: Create constants.rs with grid and color constants**

```rust
// Grid configuration: 1000x1000 = 1 million checkboxes
pub const GRID_WIDTH: u32 = 1000;
pub const GRID_HEIGHT: u32 = 1000;
pub const TOTAL_CHECKBOXES: u32 = GRID_WIDTH * GRID_HEIGHT;
pub const CELL_SIZE: f64 = 4.0;

// Zoom bounds
pub const MIN_SCALE: f64 = 0.5;
pub const MAX_SCALE: f64 = 10.0;

// Colors
pub const COLOR_CHECKED: &str = "#2ecc71";
pub const COLOR_UNCHECKED: &str = "#2c3e50";
pub const COLOR_GRID: &str = "#1a1a2e";

// SpacetimeDB
pub const DATABASE_NAME: &str = "checkboxes";
pub const SPACETIMEDB_URI_LOCAL: &str = "ws://127.0.0.1:3000";
pub const SPACETIMEDB_URI_PROD: &str = "wss://maincloud.spacetimedb.com";
```

- [ ] **Step 2: Commit**

```bash
git add frontend-rust/src/constants.rs
git commit -m "feat(frontend-rust): add constants module"
```

---

### Task 4: Create utils module with bit manipulation

**Files:**
- Create: `frontend-rust/src/utils.rs`

- [ ] **Step 1: Create utils.rs with bit manipulation functions**

```rust
use crate::constants::{GRID_WIDTH, GRID_HEIGHT, CELL_SIZE};

/// Get bit value at given index
pub fn get_bit(data: &[u8], bit_index: u32) -> bool {
    let byte_idx = (bit_index / 8) as usize;
    let bit_idx = bit_index % 8;
    if byte_idx < data.len() {
        (data[byte_idx] >> bit_idx) & 1 == 1
    } else {
        false
    }
}

/// Set bit value at given index
pub fn set_bit(data: &mut [u8], bit_index: u32, value: bool) {
    let byte_idx = (bit_index / 8) as usize;
    let bit_idx = bit_index % 8;
    if byte_idx < data.len() {
        if value {
            data[byte_idx] |= 1 << bit_idx;
        } else {
            data[byte_idx] &= !(1 << bit_idx);
        }
    }
}

/// Count total checked bits
pub fn count_bits(data: &[u8]) -> u32 {
    data.iter().map(|byte| byte.count_ones()).sum()
}

/// Convert canvas coordinates to grid column/row
/// Returns None if outside grid bounds
pub fn canvas_to_grid(
    mouse_x: f64,
    mouse_y: f64,
    offset_x: f64,
    offset_y: f64,
    scale: f64,
) -> Option<(u32, u32)> {
    let cell_size = CELL_SIZE * scale;
    let col = ((mouse_x - offset_x) / cell_size).floor() as i32;
    let row = ((mouse_y - offset_y) / cell_size).floor() as i32;

    if col >= 0 && col < GRID_WIDTH as i32 && row >= 0 && row < GRID_HEIGHT as i32 {
        Some((col as u32, row as u32))
    } else {
        None
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add frontend-rust/src/utils.rs
git commit -m "feat(frontend-rust): add utils module with bit manipulation"
```

---

### Task 5: Create state module

**Files:**
- Create: `frontend-rust/src/state.rs`

- [ ] **Step 1: Create state.rs with AppState and ConnectionStatus**

```rust
use leptos::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Error,
}

impl ConnectionStatus {
    pub fn as_class(&self) -> &'static str {
        match self {
            ConnectionStatus::Connecting => "status connecting",
            ConnectionStatus::Connected => "status connected",
            ConnectionStatus::Error => "status error",
        }
    }
}

#[derive(Clone, Copy)]
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

    // Drag state
    pub is_dragging: RwSignal<bool>,
    pub last_mouse_x: RwSignal<f64>,
    pub last_mouse_y: RwSignal<f64>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            status: RwSignal::new(ConnectionStatus::Connecting),
            status_message: RwSignal::new("Connecting...".to_string()),
            chunk_data: RwSignal::new(vec![0u8; 125_000]),
            checked_count: RwSignal::new(0),
            offset_x: RwSignal::new(0.0),
            offset_y: RwSignal::new(0.0),
            scale: RwSignal::new(1.0),
            is_dragging: RwSignal::new(false),
            last_mouse_x: RwSignal::new(0.0),
            last_mouse_y: RwSignal::new(0.0),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add frontend-rust/src/state.rs
git commit -m "feat(frontend-rust): add state module with AppState"
```

---

### Task 6: Create CSS styles

**Files:**
- Create: `frontend-rust/src/styles.css`

- [ ] **Step 1: Create styles.css (ported from index.html)**

```css
* {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: #1a1a2e;
    color: #eee;
    overflow: hidden;
}

.header {
    padding: 10px 20px;
    display: flex;
    align-items: center;
    gap: 20px;
    background: #16213e;
}

h1 {
    font-size: 1.2rem;
}

.status {
    padding: 4px 10px;
    border-radius: 4px;
    font-size: 0.85rem;
}

.status.connecting {
    background: #f39c12;
    color: #000;
}

.status.connected {
    background: #27ae60;
    color: #fff;
}

.status.error {
    background: #e74c3c;
    color: #fff;
}

.stats {
    margin-left: auto;
    font-size: 0.85rem;
    color: #888;
}

canvas {
    display: block;
    margin: 20px;
}
```

- [ ] **Step 2: Commit**

```bash
git add frontend-rust/src/styles.css
git commit -m "feat(frontend-rust): add CSS styles"
```

---

### Task 7: Create lib.rs with module declarations

**Files:**
- Create: `frontend-rust/src/lib.rs`

- [ ] **Step 1: Create lib.rs declaring all modules**

```rust
pub mod app;
pub mod components;
pub mod constants;
pub mod state;
pub mod utils;

// Note: db module will be added in Chunk 3

// Re-export for convenience
pub use app::App;
pub use state::{AppState, ConnectionStatus};
```

- [ ] **Step 2: Commit**

```bash
git add frontend-rust/src/lib.rs
git commit -m "feat(frontend-rust): add lib.rs with module declarations"
```

---

### Task 8: Create main.rs entry point

**Files:**
- Modify: `frontend-rust/src/main.rs`

- [ ] **Step 1: Update main.rs to mount the Leptos app**

```rust
use checkbox_frontend::App;
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}
```

- [ ] **Step 2: Commit**

```bash
git add frontend-rust/src/main.rs
git commit -m "feat(frontend-rust): update main.rs entry point"
```

---

## Chunk 2: Components

### Task 9: Create components module

**Files:**
- Create: `frontend-rust/src/components/mod.rs`

- [ ] **Step 1: Create components/mod.rs**

```rust
pub mod canvas;
pub mod header;

pub use canvas::CheckboxCanvas;
pub use header::Header;
```

- [ ] **Step 2: Commit**

```bash
mkdir -p frontend-rust/src/components
git add frontend-rust/src/components/mod.rs
git commit -m "feat(frontend-rust): add components module"
```

---

### Task 10: Create Header component

**Files:**
- Create: `frontend-rust/src/components/header.rs`

- [ ] **Step 1: Create header.rs with Header component**

```rust
use leptos::prelude::*;
use crate::state::AppState;
use crate::constants::TOTAL_CHECKBOXES;

#[component]
pub fn Header(state: AppState) -> impl IntoView {
    let status_class = move || state.status.get().as_class();
    let status_text = move || state.status_message.get();

    let stats_text = move || {
        let checked = state.checked_count.get();
        let scale = state.scale.get();
        format!(
            "{} / {} checked | Zoom: {:.1}x | Shift+drag to pan, scroll to zoom",
            format_number(checked),
            format_number(TOTAL_CHECKBOXES),
            scale
        )
    };

    view! {
        <div class="header">
            <h1>"1 Million Checkboxes"</h1>
            <div class=status_class>{status_text}</div>
            <div class="stats">{stats_text}</div>
        </div>
    }
}

fn format_number(n: u32) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, c);
    }
    result
}
```

- [ ] **Step 2: Commit**

```bash
git add frontend-rust/src/components/header.rs
git commit -m "feat(frontend-rust): add Header component"
```

---

### Task 11: Create Canvas component (rendering)

**Files:**
- Create: `frontend-rust/src/components/canvas.rs`

- [ ] **Step 1: Create canvas.rs with rendering logic**

```rust
use leptos::prelude::*;
use leptos::html::Canvas;
use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d, MouseEvent, WheelEvent};
use wasm_bindgen::JsCast;

use crate::state::AppState;
use crate::constants::*;
use crate::utils::{get_bit, set_bit, canvas_to_grid, count_bits};

#[component]
pub fn CheckboxCanvas(state: AppState) -> impl IntoView {
    let canvas_ref = NodeRef::<Canvas>::new();

    // Render effect - re-renders when signals change
    Effect::new(move |_| {
        let _ = state.chunk_data.get();
        let _ = state.offset_x.get();
        let _ = state.offset_y.get();
        let _ = state.scale.get();

        if let Some(canvas) = canvas_ref.get() {
            render_grid(&canvas, &state);
        }
    });

    // Window resize effect
    Effect::new(move |_| {
        if let Some(canvas) = canvas_ref.get() {
            let window = web_sys::window().expect("no window");
            let width = window.inner_width().unwrap().as_f64().unwrap() - 40.0;
            let height = window.inner_height().unwrap().as_f64().unwrap() - 120.0;
            canvas.set_width(width as u32);
            canvas.set_height(height as u32);
            render_grid(&canvas, &state);
        }
    });

    // Click handler
    let on_click = move |e: MouseEvent| {
        if e.shift_key() {
            return; // Shift+click is for panning
        }
        handle_click(e, &state, &canvas_ref);
    };

    // Pan handlers
    let on_mousedown = move |e: MouseEvent| {
        if e.button() == 0 && e.shift_key() {
            state.is_dragging.set(true);
            state.last_mouse_x.set(e.client_x() as f64);
            state.last_mouse_y.set(e.client_y() as f64);
        }
    };

    let on_mousemove = move |e: MouseEvent| {
        if state.is_dragging.get() {
            let dx = e.client_x() as f64 - state.last_mouse_x.get();
            let dy = e.client_y() as f64 - state.last_mouse_y.get();
            state.offset_x.update(|x| *x += dx);
            state.offset_y.update(|y| *y += dy);
            state.last_mouse_x.set(e.client_x() as f64);
            state.last_mouse_y.set(e.client_y() as f64);
        }
    };

    let on_mouseup = move |_: MouseEvent| {
        state.is_dragging.set(false);
    };

    let on_mouseleave = move |_: MouseEvent| {
        state.is_dragging.set(false);
    };

    // Zoom handler
    let on_wheel = move |e: WheelEvent| {
        e.prevent_default();
        handle_wheel(e, &state, &canvas_ref);
    };

    let cursor_style = move || {
        if state.is_dragging.get() {
            "cursor: grabbing"
        } else {
            "cursor: crosshair"
        }
    };

    view! {
        <canvas
            node_ref=canvas_ref
            on:click=on_click
            on:mousedown=on_mousedown
            on:mousemove=on_mousemove
            on:mouseup=on_mouseup
            on:mouseleave=on_mouseleave
            on:wheel=on_wheel
            style=cursor_style
        />
    }
}

fn render_grid(canvas: &HtmlCanvasElement, state: &AppState) {
    let ctx: CanvasRenderingContext2d = canvas
        .get_context("2d")
        .unwrap()
        .unwrap()
        .dyn_into()
        .unwrap();

    let width = canvas.width() as f64;
    let height = canvas.height() as f64;

    let chunk_data = state.chunk_data.get();
    let offset_x = state.offset_x.get();
    let offset_y = state.offset_y.get();
    let scale = state.scale.get();
    let cell_size = CELL_SIZE * scale;

    // Clear canvas
    ctx.set_fill_style_str(COLOR_GRID);
    ctx.fill_rect(0.0, 0.0, width, height);

    // Calculate visible range
    let start_col = ((-offset_x / cell_size).floor() as i32).max(0) as u32;
    let start_row = ((-offset_y / cell_size).floor() as i32).max(0) as u32;
    let end_col = (((width - offset_x) / cell_size).ceil() as u32).min(GRID_WIDTH);
    let end_row = (((height - offset_y) / cell_size).ceil() as u32).min(GRID_HEIGHT);

    // Draw visible checkboxes
    for row in start_row..end_row {
        for col in start_col..end_col {
            let bit_index = row * GRID_WIDTH + col;
            let is_checked = get_bit(&chunk_data, bit_index);

            let x = offset_x + (col as f64) * cell_size;
            let y = offset_y + (row as f64) * cell_size;

            ctx.set_fill_style_str(if is_checked { COLOR_CHECKED } else { COLOR_UNCHECKED });
            ctx.fill_rect(x + 0.5, y + 0.5, cell_size - 1.0, cell_size - 1.0);
        }
    }
}

fn handle_click(e: MouseEvent, state: &AppState, canvas_ref: &NodeRef<Canvas>) {
    let Some(canvas) = canvas_ref.get() else { return };
    let rect = canvas.get_bounding_client_rect();
    let x = e.client_x() as f64 - rect.left();
    let y = e.client_y() as f64 - rect.top();

    let offset_x = state.offset_x.get();
    let offset_y = state.offset_y.get();
    let scale = state.scale.get();

    if let Some((col, row)) = canvas_to_grid(x, y, offset_x, offset_y, scale) {
        let bit_index = row * GRID_WIDTH + col;

        // Optimistic update (local only for now - SpacetimeDB integration in Chunk 4)
        let mut data = state.chunk_data.get();
        let current = get_bit(&data, bit_index);
        set_bit(&mut data, bit_index, !current);
        let new_count = count_bits(&data);
        state.chunk_data.set(data);
        state.checked_count.set(new_count);

        // Note: Server sync will be added in Chunk 4 (Task 18)
    }
}

fn handle_wheel(e: WheelEvent, state: &AppState, canvas_ref: &NodeRef<Canvas>) {
    let Some(canvas) = canvas_ref.get() else { return };
    let rect = canvas.get_bounding_client_rect();
    let mouse_x = e.client_x() as f64 - rect.left();
    let mouse_y = e.client_y() as f64 - rect.top();

    let current_scale = state.scale.get();
    let zoom_factor = if e.delta_y() > 0.0 { 0.9 } else { 1.1 };
    let new_scale = (current_scale * zoom_factor).clamp(MIN_SCALE, MAX_SCALE);

    // Zoom toward mouse position
    let scale_change = new_scale / current_scale;
    let offset_x = state.offset_x.get();
    let offset_y = state.offset_y.get();

    state.offset_x.set(mouse_x - (mouse_x - offset_x) * scale_change);
    state.offset_y.set(mouse_y - (mouse_y - offset_y) * scale_change);
    state.scale.set(new_scale);
}
```

- [ ] **Step 2: Commit**

```bash
git add frontend-rust/src/components/canvas.rs
git commit -m "feat(frontend-rust): add CheckboxCanvas component with rendering"
```

---

### Task 12: Create App component (without SpacetimeDB)

**Files:**
- Create: `frontend-rust/src/app.rs`

- [ ] **Step 1: Create app.rs with root App component**

```rust
use leptos::prelude::*;

use crate::components::{CheckboxCanvas, Header};
use crate::state::AppState;

const STYLES: &str = include_str!("styles.css");

#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();

    view! {
        <style>{STYLES}</style>
        <Header state=state />
        <CheckboxCanvas state=state />
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add frontend-rust/src/app.rs
git commit -m "feat(frontend-rust): add App component"
```

---

### Task 13: Verify build compiles

**Files:** None (verification only)

- [ ] **Step 1: Run cargo check to verify compilation**

Run: `cd frontend-rust && cargo check 2>&1`
Expected: Should compile without errors (warnings are acceptable)

- [ ] **Step 2: Build with trunk to verify WASM compilation**

Run: `cd frontend-rust && trunk build 2>&1`
Expected: Build succeeds, outputs to `frontend-rust/dist/`

- [ ] **Step 3: Commit any necessary fixes**

If there are compilation errors, fix them and commit.

---

## Chunk 3: SpacetimeDB Integration

### Task 14: Generate Rust bindings

**Files:**
- Create: `frontend-rust/generated/` (via spacetime generate)

- [ ] **Step 1: Build backend WASM module**

Run: `cd backend && cargo build --target wasm32-unknown-unknown --release`
Expected: Creates `backend/target/wasm32-unknown-unknown/release/backend.wasm`

- [ ] **Step 2: Generate Rust bindings**

Run: `spacetime generate --lang rust --out-dir frontend-rust/generated --bin-path backend/target/wasm32-unknown-unknown/release/backend.wasm`
Expected: Creates `frontend-rust/generated/` with Rust types

- [ ] **Step 3: Examine generated code**

Run: `ls -la frontend-rust/generated/`
Expected: Should see `mod.rs` and table/reducer files

- [ ] **Step 4: Add generated module to lib.rs**

Update `frontend-rust/src/lib.rs` to include:
```rust
pub mod app;
pub mod components;
pub mod constants;
pub mod db;
pub mod state;
pub mod utils;

#[path = "../generated/mod.rs"]
pub mod generated;

pub use app::App;
pub use state::{AppState, ConnectionStatus};
```

- [ ] **Step 5: Commit generated code**

```bash
git add frontend-rust/generated frontend-rust/src/lib.rs
git commit -m "feat(frontend-rust): add SpacetimeDB generated bindings"
```

---

### Task 15: Create db module

**Files:**
- Create: `frontend-rust/src/db.rs`

- [ ] **Step 1: Create db.rs with connection logic**

```rust
use crate::constants::{DATABASE_NAME, SPACETIMEDB_URI_LOCAL, SPACETIMEDB_URI_PROD};
use crate::state::{AppState, ConnectionStatus};
use crate::utils::count_bits;

/// Check if running on localhost
pub fn is_local() -> bool {
    let window = web_sys::window().expect("no window");
    if let Ok(hostname) = window.location().hostname() {
        hostname == "localhost" || hostname == "127.0.0.1"
    } else {
        false
    }
}

/// Get the appropriate SpacetimeDB URI
pub fn get_spacetimedb_uri() -> &'static str {
    if is_local() {
        SPACETIMEDB_URI_LOCAL
    } else {
        SPACETIMEDB_URI_PROD
    }
}

/// Initialize SpacetimeDB connection (stub - full implementation in Chunk 4)
/// This sets up the status message. Actual connection logic will be added in Task 18.
pub fn init_connection(state: AppState) {
    let uri = get_spacetimedb_uri();
    let is_local_env = is_local();
    
    state.status_message.set(format!(
        "Connecting to {}...",
        if is_local_env { "local" } else { "production" }
    ));

    // Stub: Log connection attempt. Full implementation in Chunk 4 (Task 18).
    web_sys::console::log_1(&format!("SpacetimeDB URI: {}", uri).into());
    web_sys::console::log_1(&format!("Database: {}", DATABASE_NAME).into());
}

/// Update state from chunk data received from SpacetimeDB
pub fn update_from_chunk(state: &AppState, chunk_state: Vec<u8>) {
    let count = count_bits(&chunk_state);
    state.chunk_data.set(chunk_state);
    state.checked_count.set(count);
}

/// Set connection status to connected
pub fn set_connected(state: &AppState) {
    state.status.set(ConnectionStatus::Connected);
    state.status_message.set("Connected".to_string());
}

/// Set connection status to error
pub fn set_error(state: &AppState, message: &str) {
    state.status.set(ConnectionStatus::Error);
    state.status_message.set(message.to_string());
}
```

- [ ] **Step 2: Commit**

```bash
git add frontend-rust/src/db.rs
git commit -m "feat(frontend-rust): add db module with connection helpers"
```

---

### Task 16: Update App to initialize connection

**Files:**
- Modify: `frontend-rust/src/app.rs`

- [ ] **Step 1: Update app.rs to call init_connection on mount**

```rust
use leptos::prelude::*;
use std::sync::Once;

use crate::components::{CheckboxCanvas, Header};
use crate::db::init_connection;
use crate::state::AppState;

const STYLES: &str = include_str!("styles.css");

// Ensure connection is only initialized once
static INIT: Once = Once::new();

#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();

    // Initialize connection once on mount
    INIT.call_once(|| {
        init_connection(state);
    });

    view! {
        <style>{STYLES}</style>
        <Header state=state />
        <CheckboxCanvas state=state />
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add frontend-rust/src/app.rs
git commit -m "feat(frontend-rust): initialize SpacetimeDB connection on mount"
```

---

### Task 17: Verify full build

**Files:** None (verification only)

- [ ] **Step 1: Run cargo check**

Run: `cd frontend-rust && cargo check 2>&1`
Expected: Compiles without errors

- [ ] **Step 2: Run trunk build**

Run: `cd frontend-rust && trunk build 2>&1`
Expected: Build succeeds

- [ ] **Step 3: Test in browser (manual)**

Run: `cd frontend-rust && trunk serve`
Then open: `http://127.0.0.1:8080`
Expected: See header with "Connecting..." status, canvas renders empty grid

---

## Chunk 4: Full SpacetimeDB Integration

### Task 18: Examine generated SDK and implement connection

**Files:**
- Modify: `frontend-rust/src/db.rs`
- Modify: `frontend-rust/src/state.rs` (add connection storage)

This task is a discovery + implementation task. The exact API depends on the generated code.

- [ ] **Step 1: Examine generated SDK structure**

Run: `cat frontend-rust/generated/mod.rs`
Document: What types are exported? What is the DbConnection API?

Run: `ls frontend-rust/generated/`
Expected files: `mod.rs`, `checkbox_chunk.rs` (table), `update_checkbox_reducer.rs` (reducer)

- [ ] **Step 2: Add connection storage to AppState**

Update `frontend-rust/src/state.rs` to add a stored connection:

```rust
use std::rc::Rc;
use std::cell::RefCell;

// Add to AppState struct:
pub connection: StoredValue<Option<Rc<RefCell<DbConnection>>>>,

// Add to AppState::new():
connection: StoredValue::new(None),
```

- [ ] **Step 3: Implement full connection in db.rs**

Replace the stub `init_connection` with actual implementation. Pattern:

```rust
use crate::generated::{DbConnection, CheckboxChunk};
use wasm_bindgen_futures::spawn_local;

pub fn init_connection(state: AppState) {
    let uri = get_spacetimedb_uri();
    let is_local_env = is_local();
    
    state.status_message.set(format!(
        "Connecting to {}...",
        if is_local_env { "local" } else { "production" }
    ));

    spawn_local(async move {
        match connect_to_spacetimedb(uri, state).await {
            Ok(conn) => {
                // Store connection for reducer calls
                state.connection.set_value(Some(Rc::new(RefCell::new(conn))));
                set_connected(&state);
            }
            Err(e) => {
                set_error(&state, &format!("Connection failed: {}", e));
            }
        }
    });
}

async fn connect_to_spacetimedb(uri: &str, state: AppState) -> Result<DbConnection, String> {
    // Implementation depends on generated SDK API
    // Pattern from spec:
    let conn = DbConnection::builder()
        .with_uri(uri)
        .with_database_name(DATABASE_NAME)
        .on_connect(move |connection, _identity| {
            // Register table callbacks
            connection.db.checkbox_chunk.on_insert(move |row| {
                update_from_chunk(&state, row.state.clone());
            });
            
            connection.db.checkbox_chunk.on_update(move |_old, new| {
                update_from_chunk(&state, new.state.clone());
            });
            
            // Subscribe
            connection
                .subscription_builder()
                .subscribe("SELECT * FROM checkbox_chunk");
        })
        .on_disconnect(move || {
            set_error(&state, "Disconnected");
        })
        .build()
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(conn)
}
```

Note: Adjust based on actual generated API after Step 1.

- [ ] **Step 4: Add toggle_checkbox function to db.rs**

```rust
pub fn toggle_checkbox(state: &AppState, bit_index: u32, checked: bool) {
    if let Some(conn) = state.connection.get_value() {
        conn.borrow().reducers.update_checkbox(0, bit_index, checked);
    }
}
```

- [ ] **Step 5: Commit**

```bash
git add frontend-rust/src/db.rs frontend-rust/src/state.rs
git commit -m "feat(frontend-rust): implement SpacetimeDB connection"
```

---

### Task 19: Wire up canvas to use SpacetimeDB

**Files:**
- Modify: `frontend-rust/src/components/canvas.rs`

- [ ] **Step 1: Import db module in canvas.rs**

Add to imports:
```rust
use crate::db::toggle_checkbox;
```

- [ ] **Step 2: Update handle_click to call reducer**

Replace the click handler body:

```rust
fn handle_click(e: MouseEvent, state: &AppState, canvas_ref: &NodeRef<Canvas>) {
    let Some(canvas) = canvas_ref.get() else { return };
    let rect = canvas.get_bounding_client_rect();
    let x = e.client_x() as f64 - rect.left();
    let y = e.client_y() as f64 - rect.top();

    let offset_x = state.offset_x.get();
    let offset_y = state.offset_y.get();
    let scale = state.scale.get();

    if let Some((col, row)) = canvas_to_grid(x, y, offset_x, offset_y, scale) {
        let bit_index = row * GRID_WIDTH + col;

        // Optimistic update
        let mut data = state.chunk_data.get();
        let current = get_bit(&data, bit_index);
        let new_value = !current;
        set_bit(&mut data, bit_index, new_value);
        let new_count = count_bits(&data);
        state.chunk_data.set(data);
        state.checked_count.set(new_count);

        // Send to server
        toggle_checkbox(state, bit_index, new_value);
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add frontend-rust/src/components/canvas.rs
git commit -m "feat(frontend-rust): wire canvas clicks to SpacetimeDB reducer"
```

---

### Task 20: Add npm scripts for Rust frontend

**Files:**
- Modify: `package.json`

- [ ] **Step 1: Add scripts for Rust frontend**

Add to `package.json` scripts:

```json
{
  "scripts": {
    "build:frontend-rust": "cd frontend-rust && trunk build --release",
    "dev:frontend-rust": "cd frontend-rust && trunk serve",
    "generate:rust": "spacetime generate --lang rust --out-dir frontend-rust/generated --bin-path backend/target/wasm32-unknown-unknown/release/backend.wasm"
  }
}
```

- [ ] **Step 2: Commit**

```bash
git add package.json
git commit -m "chore: add npm scripts for Rust frontend"
```

---

### Task 21: Final verification

**Files:** None (verification only)

- [ ] **Step 1: Full build from clean state**

```bash
npm run build
npm run generate:rust
npm run build:frontend-rust
```

Expected: All commands succeed without errors.

- [ ] **Step 2: Start SpacetimeDB and frontend**

Terminal 1: `spacetime start` (if not already running)
Terminal 2: `npm run dev:frontend-rust`

Open: `http://127.0.0.1:8080`

- [ ] **Step 3: Verify grid renders**

Expected: See 1000x1000 grid with dark blue unchecked cells on dark background.

- [ ] **Step 4: Verify click toggles checkboxes**

Click a cell. Expected: Cell turns green, checked count increases.

- [ ] **Step 5: Verify pan with shift+drag**

Hold Shift, drag mouse. Expected: Grid pans smoothly.

- [ ] **Step 6: Verify zoom with scroll wheel**

Scroll up/down. Expected: Grid zooms in/out toward cursor.

- [ ] **Step 7: Verify connection status**

Expected: Status badge shows "Connected" (green) after initial load.

- [ ] **Step 8: Verify multi-tab sync**

Open second browser tab to same URL. Toggle checkbox in tab 1.
Expected: Change appears in tab 2 within 1 second.

- [ ] **Step 9: Final commit (if any fixes needed)**

```bash
git add -A
git commit -m "fix(frontend-rust): address verification issues"
```

---

## Summary

| Chunk | Tasks | Description |
|-------|-------|-------------|
| 1 | 1-8 | Project setup, core modules (constants, utils, state, styles) |
| 2 | 9-13 | Components (Header, Canvas, App), verify build |
| 3 | 14-17 | SpacetimeDB bindings generation, db module stub, verify build |
| 4 | 18-21 | Full SpacetimeDB integration, npm scripts, final verification |

**Total tasks:** 21
**Estimated time:** 2-3 hours

**Dependencies:**
- Trunk CLI: `cargo install trunk`
- wasm32 target: `rustup target add wasm32-unknown-unknown`
- SpacetimeDB CLI: Must be installed for `spacetime generate`

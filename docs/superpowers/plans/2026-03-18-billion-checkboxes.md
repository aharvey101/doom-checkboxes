# Billion Checkboxes Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scale the collaborative checkboxes app from 1 million to 1 billion checkboxes using chunked lazy-loading.

**Architecture:** Grid expands to 40,000×25,000 cells organized into 1,000 chunks (40×25). Frontend loads only visible chunks plus a 1-chunk buffer, subscribing/unsubscribing as viewport changes. WebGL renders each loaded chunk with per-chunk draw calls.

**Tech Stack:** Rust/WASM (Leptos), WebGL, SpacetimeDB

---

## File Structure

| File | Responsibility | Action |
|------|---------------|--------|
| `frontend-rust/src/constants.rs` | Grid and chunk dimensions | Modify |
| `frontend-rust/src/state.rs` | Multi-chunk reactive state | Modify |
| `frontend-rust/src/utils.rs` | Chunk coordinate calculations | Modify |
| `frontend-rust/src/db.rs` | Chunk subscription management | Modify |
| `frontend-rust/src/webgl.rs` | Multi-chunk rendering | Modify |
| `frontend-rust/src/components/canvas.rs` | Click handling with chunk coords | Modify |
| `frontend-rust/src/components/header.rs` | Remove count, update title | Modify |

---

## Chunk 1: Constants and State Foundation

### Task 1: Update Constants

**Files:**
- Modify: `frontend-rust/src/constants.rs`

- [ ] **Step 1: Update grid dimensions and add chunk constants**

```rust
// Grid configuration: 40,000 x 25,000 = 1 billion checkboxes
pub const GRID_WIDTH: u32 = 40_000;
pub const GRID_HEIGHT: u32 = 25_000;
pub const TOTAL_CHECKBOXES: u64 = GRID_WIDTH as u64 * GRID_HEIGHT as u64;
pub const CELL_SIZE: f64 = 4.0;

// Chunk configuration
pub const CHUNK_SIZE: u32 = 1_000;  // 1000x1000 checkboxes per chunk
pub const CHUNKS_X: u32 = 40;       // GRID_WIDTH / CHUNK_SIZE
pub const CHUNKS_Y: u32 = 25;       // GRID_HEIGHT / CHUNK_SIZE
pub const TOTAL_CHUNKS: u32 = CHUNKS_X * CHUNKS_Y;  // 1000

// Zoom bounds
pub const MIN_SCALE: f64 = 0.1;  // Lower min to see more of the larger grid
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

- [ ] **Step 2: Build to verify no compile errors**

Run: `cd frontend-rust && cargo build 2>&1 | tail -5`
Expected: Build succeeds (may have warnings about unused constants)

- [ ] **Step 3: Commit**

```bash
git add frontend-rust/src/constants.rs
git commit -m "feat: update constants for 1 billion checkboxes (40k x 25k grid)"
```

---

### Task 2: Update State for Multi-Chunk

**Files:**
- Modify: `frontend-rust/src/state.rs`

- [ ] **Step 1: Add HashMap and HashSet imports**

Add to imports at top of file:
```rust
use std::collections::{HashMap, HashSet};
```

- [ ] **Step 2: Replace chunk_data and checked_count with multi-chunk state**

Replace these fields in `AppState`:
```rust
// Remove:
// pub chunk_data: RwSignal<Vec<u8>>,
// pub checked_count: RwSignal<u32>,

// Add:
pub loaded_chunks: RwSignal<HashMap<u32, Vec<u8>>>,  // chunk_id -> data
pub loading_chunks: RwSignal<HashSet<u32>>,          // chunks being fetched
pub subscribed_chunks: RwSignal<HashSet<u32>>,       // active subscriptions
```

- [ ] **Step 3: Update AppState::new() initialization**

Replace the removed field initializations:
```rust
// Remove:
// chunk_data: RwSignal::new(vec![0u8; 125_000]),
// checked_count: RwSignal::new(0),

// Add:
loaded_chunks: RwSignal::new(HashMap::new()),
loading_chunks: RwSignal::new(HashSet::new()),
subscribed_chunks: RwSignal::new(HashSet::new()),
```

- [ ] **Step 4: Build to check for compile errors**

Run: `cd frontend-rust && cargo build 2>&1`
Expected: Compile errors about missing `chunk_data` and `checked_count` - this is expected, we'll fix them in subsequent tasks.

- [ ] **Step 5: Commit (even with expected errors in other files)**

```bash
git add frontend-rust/src/state.rs
git commit -m "feat: update state for multi-chunk storage"
```

---

### Task 3: Add Chunk Coordinate Utilities

**Files:**
- Modify: `frontend-rust/src/utils.rs`

- [ ] **Step 1: Add chunk coordinate helper functions**

Add these functions to utils.rs:
```rust
use crate::constants::{CELL_SIZE, CHUNK_SIZE, CHUNKS_X, CHUNKS_Y, GRID_WIDTH, GRID_HEIGHT};

/// Calculate chunk_id from global grid coordinates
pub fn grid_to_chunk_id(col: u32, row: u32) -> u32 {
    let chunk_x = col / CHUNK_SIZE;
    let chunk_y = row / CHUNK_SIZE;
    chunk_x + chunk_y * CHUNKS_X
}

/// Calculate local coordinates within a chunk
pub fn grid_to_local(col: u32, row: u32) -> (u32, u32) {
    (col % CHUNK_SIZE, row % CHUNK_SIZE)
}

/// Calculate bit offset within a chunk's data
pub fn local_to_bit_offset(local_col: u32, local_row: u32) -> u32 {
    local_row * CHUNK_SIZE + local_col
}

/// Calculate visible chunk range with buffer
/// Returns (min_chunk_x, min_chunk_y, max_chunk_x, max_chunk_y)
pub fn visible_chunk_range(
    offset_x: f64,
    offset_y: f64,
    scale: f64,
    canvas_w: f64,
    canvas_h: f64,
) -> (u32, u32, u32, u32) {
    let cell_size = CELL_SIZE * scale;

    // Visible grid bounds
    let min_col = ((-offset_x) / cell_size).floor().max(0.0) as u32;
    let min_row = ((-offset_y) / cell_size).floor().max(0.0) as u32;
    let max_col = ((canvas_w - offset_x) / cell_size).ceil().min(GRID_WIDTH as f64 - 1.0) as u32;
    let max_row = ((canvas_h - offset_y) / cell_size).ceil().min(GRID_HEIGHT as f64 - 1.0) as u32;

    // Convert to chunk coordinates with 1-chunk buffer
    let chunk_min_x = (min_col / CHUNK_SIZE).saturating_sub(1);
    let chunk_min_y = (min_row / CHUNK_SIZE).saturating_sub(1);
    let chunk_max_x = ((max_col / CHUNK_SIZE) + 1).min(CHUNKS_X - 1);
    let chunk_max_y = ((max_row / CHUNK_SIZE) + 1).min(CHUNKS_Y - 1);

    (chunk_min_x, chunk_min_y, chunk_max_x, chunk_max_y)
}

/// Get set of chunk IDs in visible range
pub fn visible_chunk_ids(
    offset_x: f64,
    offset_y: f64,
    scale: f64,
    canvas_w: f64,
    canvas_h: f64,
) -> std::collections::HashSet<u32> {
    let (min_cx, min_cy, max_cx, max_cy) = visible_chunk_range(offset_x, offset_y, scale, canvas_w, canvas_h);
    let mut chunks = std::collections::HashSet::new();
    for cy in min_cy..=max_cy {
        for cx in min_cx..=max_cx {
            chunks.insert(cx + cy * CHUNKS_X);
        }
    }
    chunks
}
```

- [ ] **Step 2: Build to verify**

Run: `cd frontend-rust && cargo build 2>&1 | grep -E "(error|warning:.*utils)" | head -10`
Expected: No errors in utils.rs (may still have errors in other files)

- [ ] **Step 3: Commit**

```bash
git add frontend-rust/src/utils.rs
git commit -m "feat: add chunk coordinate calculation utilities"
```

---

## Chunk 2: Header and Database Updates

### Task 4: Update Header Component

**Files:**
- Modify: `frontend-rust/src/components/header.rs`

- [ ] **Step 1: Remove checked_count usage and update title**

Replace the entire file:
```rust
use crate::state::AppState;
use leptos::prelude::*;

#[component]
pub fn Header(state: AppState) -> impl IntoView {
    let status_class = move || state.status.get().as_class();
    let status_text = move || state.status_message.get();

    let stats_text = move || {
        let scale = state.scale.get();
        format!(
            "Zoom: {:.1}x | Shift+drag to pan, scroll to zoom",
            scale
        )
    };

    view! {
        <div class="header">
            <h1>"1 Billion Checkboxes"</h1>
            <div class=status_class>{status_text}</div>
            <div class="stats">{stats_text}</div>
        </div>
    }
}
```

- [ ] **Step 2: Build to verify**

Run: `cd frontend-rust && cargo build 2>&1 | grep -E "header" | head -5`
Expected: No errors in header.rs

- [ ] **Step 3: Commit**

```bash
git add frontend-rust/src/components/header.rs
git commit -m "feat: update header for billion checkboxes, remove count display"
```

---

### Task 5: Update Database Module for Multi-Chunk

**Files:**
- Modify: `frontend-rust/src/db.rs`

- [ ] **Step 1: Update imports**

Add/update imports at top:
```rust
use crate::constants::{CHUNK_SIZE, CHUNKS_X, GRID_WIDTH};
use crate::utils::{grid_to_chunk_id, local_to_bit_offset, grid_to_local};
use std::collections::HashSet;
```

- [ ] **Step 2: Update on_chunk_insert callback**

Find the `on_chunk_insert` callback and replace it to handle any chunk_id:
```rust
// On chunk insert (initial data load)
let state_insert = state;
client_mut.on_chunk_insert(Box::new(move |row_bytes: &[u8]| {
    if let Some(chunk) = CheckboxChunk::from_bsatn(row_bytes) {
        web_sys::console::log_1(
            &format!(
                "Chunk {} received, {} bytes, version {}",
                chunk.chunk_id,
                chunk.state.len(),
                chunk.version
            )
            .into(),
        );

        // Store chunk data
        state_insert.loaded_chunks.update(|chunks| {
            chunks.insert(chunk.chunk_id, chunk.state);
        });
        
        // Mark as no longer loading, add to subscribed
        state_insert.loading_chunks.update(|loading| {
            loading.remove(&chunk.chunk_id);
        });
        state_insert.subscribed_chunks.update(|subs| {
            subs.insert(chunk.chunk_id);
        });
        
        // Trigger render
        state_insert.render_version.update(|v| *v += 1);
    }
}));
```

- [ ] **Step 3: Update on_chunk_update callback**

Find the `on_chunk_update` callback and replace:
```rust
// On chunk update (state changes from other clients)
let state_update = state;
client_mut.on_chunk_update(Box::new(move |_old_bytes: &[u8], new_bytes: &[u8]| {
    if let Some(chunk) = CheckboxChunk::from_bsatn(new_bytes) {
        // Only update if we have this chunk loaded
        let should_update = state_update.loaded_chunks.with_untracked(|chunks| {
            if let Some(existing) = chunks.get(&chunk.chunk_id) {
                existing != &chunk.state
            } else {
                false
            }
        });
        
        if should_update {
            web_sys::console::log_1(
                &format!(
                    "Chunk {} updated from server, version {}",
                    chunk.chunk_id, chunk.version
                )
                .into(),
            );
            state_update.loaded_chunks.update(|chunks| {
                chunks.insert(chunk.chunk_id, chunk.state);
            });
            // Trigger full re-render for server updates
            state_update.render_version.update(|v| *v += 1);
        }
    }
}));
```

- [ ] **Step 4: Update toggle_checkbox function**

Replace the existing `toggle_checkbox` function:
```rust
/// Toggle a checkbox at the given grid position
/// Returns the new checked state for immediate visual feedback
pub fn toggle_checkbox(state: AppState, col: u32, row: u32) -> Option<bool> {
    let chunk_id = grid_to_chunk_id(col, row);
    let (local_col, local_row) = grid_to_local(col, row);
    let bit_offset = local_to_bit_offset(local_col, local_row) as usize;

    // Get current value and toggle
    let current_value = state.loaded_chunks.with_untracked(|chunks| {
        chunks.get(&chunk_id).map(|data| get_bit(data, bit_offset)).unwrap_or(false)
    });
    let new_value = !current_value;

    // Optimistic update
    state.loaded_chunks.update(|chunks| {
        if let Some(data) = chunks.get_mut(&chunk_id) {
            set_bit(data, bit_offset, new_value);
        }
    });

    // Send to server
    let client = CLIENT.borrow();
    if let Some(ref client) = *client {
        call_reducer(
            client,
            "update_checkbox",
            &(chunk_id, bit_offset as u32, new_value),
        );
    }

    Some(new_value)
}
```

- [ ] **Step 5: Remove count_checked function and its usages**

Find and remove the `count_checked` function (no longer needed):
```rust
// DELETE this function:
// fn count_checked(data: &[u8]) -> u32 { ... }
```

Also remove any calls to `count_checked` and `checked_count.set(...)` in the callbacks.

- [ ] **Step 6: Add subscribe_to_chunks function**

Add this function for subscribing to visible chunks:
```rust
/// Subscribe to a set of chunks
pub fn subscribe_to_chunks(state: AppState, chunk_ids: HashSet<u32>) {
    let client = CLIENT.borrow();
    if let Some(ref client) = *client {
        for chunk_id in chunk_ids {
            // Skip if already subscribed or loading
            if state.subscribed_chunks.with_untracked(|s| s.contains(&chunk_id)) {
                continue;
            }
            if state.loading_chunks.with_untracked(|l| l.contains(&chunk_id)) {
                continue;
            }
            
            // Mark as loading
            state.loading_chunks.update(|loading| {
                loading.insert(chunk_id);
            });
            
            // Subscribe via SpacetimeDB
            let query = format!("SELECT * FROM checkbox_chunk WHERE chunk_id = {}", chunk_id);
            subscribe(client, &[&query]);
            
            web_sys::console::log_1(&format!("Subscribing to chunk {}", chunk_id).into());
        }
    }
}
```

- [ ] **Step 7: Build to check progress**

Run: `cd frontend-rust && cargo build 2>&1 | grep -c "^error"`
Expected: Some errors remain (canvas.rs, webgl.rs still need updates)

- [ ] **Step 8: Commit**

```bash
git add frontend-rust/src/db.rs
git commit -m "feat: update db module for multi-chunk subscriptions"
```

---

## Chunk 3: WebGL Renderer Updates

### Task 6: Update WebGL Renderer for Multi-Chunk

**Files:**
- Modify: `frontend-rust/src/webgl.rs`

- [ ] **Step 1: Update imports**

Add chunk-related imports:
```rust
use crate::constants::{CELL_SIZE, CHUNK_SIZE, CHUNKS_X, CHUNKS_Y, GRID_WIDTH, GRID_HEIGHT, ...};
use crate::utils::visible_chunk_range;
use std::collections::HashMap;
```

- [ ] **Step 2: Update render function signature**

Find the `render` function and update its signature to accept HashMap:
```rust
pub fn render(
    &self,
    canvas: &HtmlCanvasElement,
    loaded_chunks: &HashMap<u32, Vec<u8>>,
    offset_x: f64,
    offset_y: f64,
    scale: f64,
) {
```

- [ ] **Step 3: Implement multi-chunk rendering**

Replace the render function body:
```rust
pub fn render(
    &self,
    canvas: &HtmlCanvasElement,
    loaded_chunks: &HashMap<u32, Vec<u8>>,
    offset_x: f64,
    offset_y: f64,
    scale: f64,
) {
    let width = canvas.width() as f64;
    let height = canvas.height() as f64;

    self.gl.viewport(0, 0, width as i32, height as i32);

    // Clear with grid background
    let (bg_r, bg_g, bg_b) = parse_hex_color(COLOR_GRID);
    self.gl.clear_color(bg_r, bg_g, bg_b, 1.0);
    self.gl.clear(GL::COLOR_BUFFER_BIT);

    // Calculate visible chunk range
    let (min_cx, min_cy, max_cx, max_cy) = visible_chunk_range(offset_x, offset_y, scale, width, height);

    // Render each loaded chunk in visible range
    for cy in min_cy..=max_cy {
        for cx in min_cx..=max_cx {
            let chunk_id = cx + cy * CHUNKS_X;
            if let Some(chunk_data) = loaded_chunks.get(&chunk_id) {
                self.render_chunk(canvas, chunk_id, chunk_data, offset_x, offset_y, scale);
            }
            // Unloaded chunks just show background (already cleared)
        }
    }
}
```

- [ ] **Step 4: Add render_chunk helper function**

Add this new function:
```rust
fn render_chunk(
    &self,
    canvas: &HtmlCanvasElement,
    chunk_id: u32,
    chunk_data: &[u8],
    offset_x: f64,
    offset_y: f64,
    scale: f64,
) {
    let width = canvas.width() as f64;
    let height = canvas.height() as f64;

    // Calculate chunk's world position
    let chunk_x = chunk_id % CHUNKS_X;
    let chunk_y = chunk_id / CHUNKS_X;
    let chunk_offset_x = offset_x + (chunk_x * CHUNK_SIZE) as f64 * CELL_SIZE * scale;
    let chunk_offset_y = offset_y + (chunk_y * CHUNK_SIZE) as f64 * CELL_SIZE * scale;

    // Upload chunk texture
    self.gl.bind_texture(GL::TEXTURE_2D, Some(&self.state_texture));
    self.gl
        .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            GL::TEXTURE_2D,
            0,
            GL::LUMINANCE as i32,
            500,  // 125000 bytes = 500x250 texture
            250,
            0,
            GL::LUMINANCE,
            GL::UNSIGNED_BYTE,
            Some(chunk_data),
        )
        .expect("Failed to upload texture");

    // Set uniforms for this chunk's position
    self.gl.uniform2f(Some(&self.u_resolution), width as f32, height as f32);
    self.gl.uniform2f(Some(&self.u_offset), chunk_offset_x as f32, chunk_offset_y as f32);
    self.gl.uniform1f(Some(&self.u_scale), scale as f32);

    // Draw
    self.gl.draw_arrays(GL::TRIANGLES, 0, 6);
}
```

- [ ] **Step 5: Update render_cell_immediate for chunk coordinates**

Update the function to handle global coordinates across chunks:
```rust
pub fn render_cell_immediate(
    &self,
    canvas: &HtmlCanvasElement,
    col: u32,
    row: u32,
    is_checked: bool,
    offset_x: f64,
    offset_y: f64,
    scale: f64,
) {
    let cell_size = CELL_SIZE * scale;

    // Calculate cell position on screen (global coordinates)
    let x = offset_x + (col as f64) * cell_size;
    let y = offset_y + (row as f64) * cell_size;

    let width = canvas.width() as f64;
    let height = canvas.height() as f64;

    // Skip if outside visible area
    if x + cell_size < 0.0 || x > width || y + cell_size < 0.0 || y > height {
        return;
    }

    // Use scissor test to only draw in the cell area
    self.gl.enable(GL::SCISSOR_TEST);

    // WebGL scissor Y is from bottom, need to flip
    let scissor_x = (x + 0.5) as i32;
    let scissor_y = (height - y - cell_size + 0.5) as i32;
    let scissor_w = (cell_size - 1.0).max(1.0) as i32;
    let scissor_h = (cell_size - 1.0).max(1.0) as i32;

    self.gl.scissor(scissor_x, scissor_y, scissor_w, scissor_h);

    // Clear with the cell color
    let (r, g, b) = if is_checked {
        parse_hex_color(COLOR_CHECKED)
    } else {
        parse_hex_color(COLOR_UNCHECKED)
    };

    self.gl.clear_color(r, g, b, 1.0);
    self.gl.clear(GL::COLOR_BUFFER_BIT);

    self.gl.disable(GL::SCISSOR_TEST);
}
```

- [ ] **Step 6: Build to check progress**

Run: `cd frontend-rust && cargo build 2>&1 | grep -c "^error"`
Expected: Fewer errors (mainly canvas.rs remaining)

- [ ] **Step 7: Commit**

```bash
git add frontend-rust/src/webgl.rs
git commit -m "feat: update WebGL renderer for multi-chunk rendering"
```

---

## Chunk 4: Canvas Component Updates

### Task 7: Update Canvas Component

**Files:**
- Modify: `frontend-rust/src/components/canvas.rs`

- [ ] **Step 1: Update imports**

Add chunk-related imports:
```rust
use crate::utils::visible_chunk_ids;
use crate::db::subscribe_to_chunks;
```

- [ ] **Step 2: Update render call in request_render closure**

Find the render call and update to pass HashMap:
```rust
if let Some(ref r) = *renderer_borrow {
    let loaded_chunks = state_copy.loaded_chunks.get_untracked();
    let offset_x = state_copy.offset_x.get_untracked();
    let offset_y = state_copy.offset_y.get_untracked();
    let scale = state_copy.scale.get_untracked();
    r.render(&canvas, &loaded_chunks, offset_x, offset_y, scale);
}
```

- [ ] **Step 3: Add chunk subscription effect**

Add a new Effect that subscribes to chunks when viewport changes:
```rust
// Chunk subscription effect - subscribe to visible chunks when viewport changes
Effect::new(move |_| {
    let offset_x = state.offset_x.get();
    let offset_y = state.offset_y.get();
    let scale = state.scale.get();
    
    if let Some(canvas) = canvas_ref.get() {
        let width = canvas.width() as f64;
        let height = canvas.height() as f64;
        
        let visible = visible_chunk_ids(offset_x, offset_y, scale, width, height);
        subscribe_to_chunks(state, visible);
    }
});
```

- [ ] **Step 4: Update render effect to also track loaded_chunks**

Update the viewport render effect:
```rust
// Render effect for viewport changes and chunk updates
let request_render_effect = request_render.clone();
Effect::new(move |_| {
    // Track viewport changes
    let _ = state.offset_x.get();
    let _ = state.offset_y.get();
    let _ = state.scale.get();
    // Track chunk data changes
    let _ = state.loaded_chunks.get();
    // Track server updates
    let _ = state.render_version.get();

    request_render_effect();
});
```

- [ ] **Step 5: Build the project**

Run: `cd frontend-rust && cargo build 2>&1`
Expected: Build succeeds

- [ ] **Step 6: Commit**

```bash
git add frontend-rust/src/components/canvas.rs
git commit -m "feat: update canvas for multi-chunk viewport and subscriptions"
```

---

### Task 8: Build and Test Locally

**Files:**
- All frontend files

- [ ] **Step 1: Full rebuild**

Run: `cd frontend-rust && cargo build --release 2>&1 | tail -10`
Expected: Build succeeds

- [ ] **Step 2: Start local SpacetimeDB (if available)**

Run: `spacetime start 2>&1 &` (or skip if not available locally)

- [ ] **Step 3: Build with trunk**

Run: `cd frontend-rust && trunk build 2>&1`
Expected: Build succeeds

- [ ] **Step 4: Commit final state**

```bash
git add -A
git commit -m "feat: complete billion checkboxes implementation" --allow-empty
```

---

## Summary

This plan implements 1 billion checkboxes by:

1. **Task 1:** Update constants for 40k×25k grid with 1000 chunks
2. **Task 2:** Update state for HashMap-based multi-chunk storage  
3. **Task 3:** Add chunk coordinate calculation utilities
4. **Task 4:** Update header (remove count, new title)
5. **Task 5:** Update database for multi-chunk subscriptions
6. **Task 6:** Update WebGL for per-chunk rendering
7. **Task 7:** Update canvas for viewport-based chunk loading
8. **Task 8:** Final build and test

Each task produces a commit, allowing incremental progress and easy rollback.

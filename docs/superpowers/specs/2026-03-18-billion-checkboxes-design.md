# Billion Checkboxes Design

Scale the collaborative checkboxes app from 1 million to 1 billion checkboxes using chunked lazy-loading.

## Summary

- **Grid:** 40,000 × 25,000 = 1,000,000,000 checkboxes
- **Chunks:** 40 × 25 = 1,000 chunks, each 1,000 × 1,000 (1M checkboxes, 125KB)
- **Loading:** Viewport + 1-chunk buffer, on-demand chunk creation
- **Count display:** Removed (no aggregation overhead)

## Grid Layout

| Property | Value |
|----------|-------|
| Grid width | 40,000 cells |
| Grid height | 25,000 cells |
| Total checkboxes | 1,000,000,000 |
| Chunk dimensions | 1,000 × 1,000 |
| Chunks horizontally | 40 |
| Chunks vertically | 25 |
| Total chunks | 1,000 |
| Chunk size (bytes) | 125,000 (125KB) |
| Max total storage | 125MB |

## Frontend Changes

### Constants (`constants.rs`)

```rust
// Grid configuration: 40,000 x 25,000 = 1 billion checkboxes
pub const GRID_WIDTH: u32 = 40_000;
pub const GRID_HEIGHT: u32 = 25_000;
pub const TOTAL_CHECKBOXES: u64 = GRID_WIDTH as u64 * GRID_HEIGHT as u64;

// Chunk configuration
pub const CHUNK_SIZE: u32 = 1_000;  // 1000x1000 checkboxes per chunk
pub const CHUNKS_X: u32 = 40;       // GRID_WIDTH / CHUNK_SIZE
pub const CHUNKS_Y: u32 = 25;       // GRID_HEIGHT / CHUNK_SIZE
pub const TOTAL_CHUNKS: u32 = CHUNKS_X * CHUNKS_Y;  // 1000
```

### State (`state.rs`)

Replace single chunk with multi-chunk tracking:

```rust
// Remove
pub chunk_data: RwSignal<Vec<u8>>,
pub checked_count: RwSignal<u32>,

// Add
pub loaded_chunks: RwSignal<HashMap<u32, Vec<u8>>>,  // chunk_id -> data
pub loading_chunks: RwSignal<HashSet<u32>>,          // chunks being fetched
```

### Chunk Coordinate System

```
chunk_id = chunk_x + chunk_y * CHUNKS_X

Where:
  chunk_x = global_col / CHUNK_SIZE
  chunk_y = global_row / CHUNK_SIZE
  
Local coordinates within chunk:
  local_col = global_col % CHUNK_SIZE
  local_row = global_row % CHUNK_SIZE
  bit_offset = local_row * CHUNK_SIZE + local_col
```

### Visible Chunk Calculation

Given viewport state (offset_x, offset_y, scale, canvas_width, canvas_height):

```rust
fn visible_chunk_range(
    offset_x: f64, offset_y: f64, 
    scale: f64, 
    canvas_w: f64, canvas_h: f64
) -> (u32, u32, u32, u32) {
    let cell_size = CELL_SIZE * scale;
    
    // Visible grid bounds
    let min_col = ((-offset_x) / cell_size).floor().max(0.0) as u32;
    let min_row = ((-offset_y) / cell_size).floor().max(0.0) as u32;
    let max_col = ((canvas_w - offset_x) / cell_size).ceil().min(GRID_WIDTH as f64) as u32;
    let max_row = ((canvas_h - offset_y) / cell_size).ceil().min(GRID_HEIGHT as f64) as u32;
    
    // Convert to chunk coordinates with 1-chunk buffer
    let chunk_min_x = (min_col / CHUNK_SIZE).saturating_sub(1);
    let chunk_min_y = (min_row / CHUNK_SIZE).saturating_sub(1);
    let chunk_max_x = ((max_col / CHUNK_SIZE) + 1).min(CHUNKS_X - 1);
    let chunk_max_y = ((max_row / CHUNK_SIZE) + 1).min(CHUNKS_Y - 1);
    
    (chunk_min_x, chunk_min_y, chunk_max_x, chunk_max_y)
}
```

### Chunk Subscription Management

Track currently subscribed chunks. When visible range changes:

1. Calculate newly visible chunks (need to subscribe)
2. Calculate no-longer-visible chunks (can unsubscribe)
3. Subscribe to new chunks via SpacetimeDB
4. Unsubscribe from old chunks to free memory

### WebGL Renderer Changes

The renderer needs to handle multiple chunks:

```rust
pub fn render(
    &self,
    canvas: &HtmlCanvasElement,
    loaded_chunks: &HashMap<u32, Vec<u8>>,
    offset_x: f64,
    offset_y: f64,
    scale: f64,
) {
    // For each visible chunk:
    //   1. If loaded, upload texture and render
    //   2. If not loaded, render as background color (empty grid)
}
```

Option A: Single texture atlas (complex, limited by max texture size)
Option B: Render each chunk separately (simpler, multiple draw calls)

Recommend Option B for simplicity. With viewport culling, only ~4-12 chunks visible at typical zoom levels.

### Click Handling

```rust
fn on_click(global_col: u32, global_row: u32) {
    let chunk_x = global_col / CHUNK_SIZE;
    let chunk_y = global_row / CHUNK_SIZE;
    let chunk_id = chunk_x + chunk_y * CHUNKS_X;
    
    let local_col = global_col % CHUNK_SIZE;
    let local_row = global_row % CHUNK_SIZE;
    let bit_offset = local_row * CHUNK_SIZE + local_col;
    
    // Optimistic local update
    update_local_chunk(chunk_id, bit_offset, new_value);
    
    // Immediate visual feedback
    render_cell_immediate(...);
    
    // Send to server
    call_reducer("update_checkbox", chunk_id, bit_offset, new_value);
}
```

## Backend Changes

### Schema

No changes needed. Existing `CheckboxChunk` table works:

```rust
#[table(accessor = checkbox_chunk, public)]
pub struct CheckboxChunk {
    #[primary_key]
    pub chunk_id: u32,      // 0-999 for billion checkboxes
    pub state: Vec<u8>,     // 125KB per chunk
    pub version: u64,
}
```

### Reducers

No changes needed. Existing `update_checkbox` already:
- Creates chunk on first access
- Updates bit at offset
- Increments version

### Removed

Remove checked count display from UI (no `checked_count` signal, no count aggregation).

## Migration

No data migration needed:
- Existing chunk 0 contains the original 1M checkboxes
- Grid expands to include new chunk coordinates
- Empty chunks created on-demand when users explore new areas

## Performance Considerations

### Memory

- Each loaded chunk: 125KB
- Typical viewport: 4-12 chunks visible
- With buffer: ~500KB - 1.5MB in memory
- Well within browser limits

### Network

- Initial load: 1-4 chunks (~125-500KB)
- Panning: subscribe/unsubscribe as chunks enter/leave viewport
- Chunk updates: only subscribed chunks receive updates

### Rendering

- WebGL renders only loaded chunks
- Unloaded areas show as grid background
- Immediate cell rendering unchanged

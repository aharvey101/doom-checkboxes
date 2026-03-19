// SpacetimeDB backend for collaborative checkboxes - v4.0 (infinite chunks with RLE compression)

use spacetimedb::{reducer, table, ReducerContext, SpacetimeType, Table};

/// Chunk size: 1 million checkboxes per chunk (1000x1000)
const CHECKBOXES_PER_CHUNK: usize = 1_000_000;

/// Legacy chunk data size (4 bytes per checkbox) - kept for reference
const CHUNK_DATA_SIZE: usize = 4_000_000;

// === Chunk Addressing ===
// Chunks are addressed by (x, y) signed coordinates, encoded into a single i64

/// Encode chunk coordinates to a single i64 for use as primary key
fn chunk_coords_to_id(x: i32, y: i32) -> i64 {
    ((x as i64) << 32) | ((y as u32) as i64)
}

/// Decode chunk id back to (x, y) coordinates
fn chunk_id_to_coords(id: i64) -> (i32, i32) {
    let x = (id >> 32) as i32;
    let y = id as i32;
    (x, y)
}

// === RLE Compression ===
// Format: [count_high: u8, count_low: u8, value: u8] repeated
// count is u16 (max 65535), value is palette index (0 = unchecked)

/// RLE encode checkbox data (1 byte per checkbox: palette index)
fn rle_encode(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut current_value = data[0];
    let mut count: u16 = 1;

    for &value in &data[1..] {
        if value == current_value && count < 65535 {
            count += 1;
        } else {
            // Emit run
            result.push((count >> 8) as u8);
            result.push(count as u8);
            result.push(current_value);
            current_value = value;
            count = 1;
        }
    }

    // Emit final run
    result.push((count >> 8) as u8);
    result.push(count as u8);
    result.push(current_value);

    result
}

/// RLE decode to checkbox data
fn rle_decode(encoded: &[u8], expected_len: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(expected_len);
    let mut i = 0;

    while i + 2 < encoded.len() {
        let count = ((encoded[i] as u16) << 8) | (encoded[i + 1] as u16);
        let value = encoded[i + 2];

        for _ in 0..count {
            result.push(value);
            if result.len() >= expected_len {
                return result;
            }
        }
        i += 3;
    }

    // Pad with zeros if needed
    result.resize(expected_len, 0);
    result
}

// === Palette ===
// Stores up to 255 RGB colors (index 0 = unchecked)
// Format: [count: u8, r1, g1, b1, r2, g2, b2, ...]

struct Palette {
    colors: Vec<(u8, u8, u8)>, // RGB entries, index 0 unused (unchecked)
}

impl Palette {
    fn new() -> Self {
        Palette { colors: Vec::new() }
    }

    fn len(&self) -> usize {
        self.colors.len()
    }

    /// Get or add a color, returns palette index (1-255)
    fn get_or_add(&mut self, r: u8, g: u8, b: u8) -> u8 {
        // Check if color exists
        for (i, &(cr, cg, cb)) in self.colors.iter().enumerate() {
            if cr == r && cg == g && cb == b {
                return (i + 1) as u8; // +1 because index 0 is unchecked
            }
        }

        // Add new color if space available
        if self.colors.len() < 255 {
            self.colors.push((r, g, b));
            return self.colors.len() as u8;
        }

        // Palette full, return last color index
        255
    }

    /// Get RGB for a palette index
    fn get_color(&self, index: u8) -> Option<(u8, u8, u8)> {
        if index == 0 || index as usize > self.colors.len() {
            None
        } else {
            Some(self.colors[index as usize - 1])
        }
    }

    /// Encode palette to bytes
    fn encode(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(1 + self.colors.len() * 3);
        result.push(self.colors.len() as u8);
        for &(r, g, b) in &self.colors {
            result.push(r);
            result.push(g);
            result.push(b);
        }
        result
    }

    /// Decode palette from bytes
    fn decode(data: &[u8]) -> Self {
        if data.is_empty() {
            return Palette::new();
        }

        let count = data[0] as usize;
        let mut colors = Vec::with_capacity(count);

        let mut i = 1;
        while i + 2 < data.len() && colors.len() < count {
            colors.push((data[i], data[i + 1], data[i + 2]));
            i += 3;
        }

        Palette { colors }
    }
}

/// A single checkbox update for batch operations (with color)
#[derive(SpacetimeType)]
pub struct CheckboxUpdate {
    pub chunk_id: i64,    // Changed to i64 for infinite chunks
    pub cell_offset: u32, // Which checkbox in the chunk (0 to 999,999)
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub checked: bool,
}

/// Stores checkbox state in chunks of 1 million checkboxes each  
/// Using i64 chunk_id to support infinite chunks with signed coordinates
/// Currently stores uncompressed 4MB per chunk (legacy format)
/// TODO: Switch to compressed format (palette + RLE)
#[table(accessor = checkbox_chunk, public)]
pub struct CheckboxChunk {
    #[primary_key]
    pub chunk_id: i64, // Changed to i64 for infinite chunks
    pub state: Vec<u8>, // 4MB blob for 1M checkboxes with colors
    pub version: u64,   // For tracking updates
}

/// Set a checkbox with color at the given position
fn set_checkbox(data: &mut [u8], cell_index: usize, r: u8, g: u8, b: u8, checked: bool) {
    let byte_idx = cell_index * 4;
    if byte_idx + 3 < data.len() {
        data[byte_idx] = r;
        data[byte_idx + 1] = g;
        data[byte_idx + 2] = b;
        data[byte_idx + 3] = if checked { 0xFF } else { 0x00 };
    }
}

/// Update a single checkbox with color in a chunk
#[reducer]
pub fn update_checkbox(
    ctx: &ReducerContext,
    chunk_id: i64,
    cell_offset: u32,
    r: u8,
    g: u8,
    b: u8,
    checked: bool,
) {
    // Try to find existing chunk by primary key
    if let Some(mut row) = ctx.db.checkbox_chunk().chunk_id().find(chunk_id) {
        set_checkbox(&mut row.state, cell_offset as usize, r, g, b, checked);
        row.version += 1;
        ctx.db.checkbox_chunk().chunk_id().update(row);
        return;
    }

    // If chunk doesn't exist, create it and set the checkbox
    let mut new_chunk = CheckboxChunk {
        chunk_id,
        state: vec![0u8; CHUNK_DATA_SIZE],
        version: 0,
    };
    set_checkbox(&mut new_chunk.state, cell_offset as usize, r, g, b, checked);
    ctx.db.checkbox_chunk().insert(new_chunk);
}

/// Batch update multiple checkboxes at once (with colors)
/// Each update contains chunk_id, cell_offset, RGB color, and checked state
#[reducer]
pub fn batch_update_checkboxes(ctx: &ReducerContext, updates: Vec<CheckboxUpdate>) {
    use std::collections::HashMap;

    // Group updates by chunk_id
    let mut chunk_updates: HashMap<i64, Vec<(u32, u8, u8, u8, bool)>> = HashMap::new();

    for update in updates {
        chunk_updates.entry(update.chunk_id).or_default().push((
            update.cell_offset,
            update.r,
            update.g,
            update.b,
            update.checked,
        ));
    }

    // Apply all updates per chunk
    for (chunk_id, updates) in chunk_updates {
        if let Some(mut row) = ctx.db.checkbox_chunk().chunk_id().find(chunk_id) {
            for (cell_offset, r, g, b, checked) in updates {
                set_checkbox(&mut row.state, cell_offset as usize, r, g, b, checked);
            }
            row.version += 1;
            ctx.db.checkbox_chunk().chunk_id().update(row);
        } else {
            // Create new chunk
            let mut new_chunk = CheckboxChunk {
                chunk_id,
                state: vec![0u8; CHUNK_DATA_SIZE],
                version: 0,
            };
            for (cell_offset, r, g, b, checked) in updates {
                set_checkbox(&mut new_chunk.state, cell_offset as usize, r, g, b, checked);
            }
            ctx.db.checkbox_chunk().insert(new_chunk);
        }
    }
}

/// Add a new chunk for expanding to additional checkboxes
#[reducer]
pub fn add_chunk(ctx: &ReducerContext, chunk_id: i64) {
    let new_chunk = CheckboxChunk {
        chunk_id,
        state: vec![0u8; CHUNK_DATA_SIZE],
        version: 0,
    };
    ctx.db.checkbox_chunk().insert(new_chunk);
}

/// Clear all checkbox data (useful for testing)
#[reducer]
pub fn clear_all_checkboxes(ctx: &ReducerContext) {
    let chunk_ids: Vec<i64> = ctx
        .db
        .checkbox_chunk()
        .iter()
        .map(|row| row.chunk_id)
        .collect();

    for chunk_id in chunk_ids {
        ctx.db.checkbox_chunk().chunk_id().delete(chunk_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_checkbox() {
        let mut data = vec![0u8; 40]; // 10 checkboxes

        // Set first checkbox to red, checked
        set_checkbox(&mut data, 0, 255, 0, 0, true);
        assert_eq!(data[0], 255); // R
        assert_eq!(data[1], 0); // G
        assert_eq!(data[2], 0); // B
        assert_eq!(data[3], 0xFF); // checked

        // Set second checkbox to green, checked
        set_checkbox(&mut data, 1, 0, 255, 0, true);
        assert_eq!(data[4], 0); // R
        assert_eq!(data[5], 255); // G
        assert_eq!(data[6], 0); // B
        assert_eq!(data[7], 0xFF); // checked

        // Set first checkbox to unchecked (color should still be set)
        set_checkbox(&mut data, 0, 100, 100, 100, false);
        assert_eq!(data[0], 100);
        assert_eq!(data[1], 100);
        assert_eq!(data[2], 100);
        assert_eq!(data[3], 0x00); // unchecked
    }

    #[test]
    fn test_chunk_size() {
        let chunk = CheckboxChunk {
            chunk_id: 0,
            state: vec![0u8; CHUNK_DATA_SIZE],
            version: 0,
        };
        // 4 bytes per checkbox, 1 million checkboxes
        assert_eq!(chunk.state.len(), 4_000_000);
        assert_eq!(chunk.state.len() / 4, 1_000_000);
    }

    // === Chunk Addressing Tests ===

    #[test]
    fn test_chunk_coords_to_id_origin() {
        // Chunk at origin (0, 0) should encode to 0
        let id = chunk_coords_to_id(0, 0);
        assert_eq!(id, 0i64);
    }

    #[test]
    fn test_chunk_coords_to_id_positive() {
        // Chunk at (1, 0)
        let id = chunk_coords_to_id(1, 0);
        assert_eq!(id, 1i64 << 32);

        // Chunk at (0, 1)
        let id = chunk_coords_to_id(0, 1);
        assert_eq!(id, 1i64);

        // Chunk at (1, 1)
        let id = chunk_coords_to_id(1, 1);
        assert_eq!(id, (1i64 << 32) | 1);
    }

    #[test]
    fn test_chunk_coords_to_id_negative() {
        // Chunk at (-1, 0)
        let id = chunk_coords_to_id(-1, 0);
        let (x, y) = chunk_id_to_coords(id);
        assert_eq!((x, y), (-1, 0));

        // Chunk at (0, -1)
        let id = chunk_coords_to_id(0, -1);
        let (x, y) = chunk_id_to_coords(id);
        assert_eq!((x, y), (0, -1));

        // Chunk at (-1, -1)
        let id = chunk_coords_to_id(-1, -1);
        let (x, y) = chunk_id_to_coords(id);
        assert_eq!((x, y), (-1, -1));
    }

    #[test]
    fn test_chunk_coords_roundtrip() {
        // Test various coordinates roundtrip correctly
        let test_coords = [
            (0, 0),
            (1, 0),
            (0, 1),
            (1, 1),
            (-1, 0),
            (0, -1),
            (-1, -1),
            (1000, 2000),
            (-1000, -2000),
            (i32::MAX, i32::MAX),
            (i32::MIN, i32::MIN),
        ];

        for (x, y) in test_coords {
            let id = chunk_coords_to_id(x, y);
            let (rx, ry) = chunk_id_to_coords(id);
            assert_eq!((rx, ry), (x, y), "Roundtrip failed for ({}, {})", x, y);
        }
    }

    // === RLE Compression Tests ===

    #[test]
    fn test_rle_encode_empty_chunk() {
        // All unchecked (all zeros) should compress to minimal size
        let data = vec![0u8; 1_000_000];
        let encoded = rle_encode(&data);

        // Should be very small: just one run of 1M zeros
        // Format: [count_high, count_low, value] = 3 bytes per run
        // But count is u16 max 65535, so we need multiple runs
        // 1M / 65535 ≈ 16 runs = 48 bytes
        assert!(
            encoded.len() < 100,
            "Empty chunk should compress to < 100 bytes, got {}",
            encoded.len()
        );
    }

    #[test]
    fn test_rle_encode_single_checkbox() {
        // One checkbox checked at position 0
        let mut data = vec![0u8; 100];
        data[0] = 1; // palette index 1

        let encoded = rle_encode(&data);
        let decoded = rle_decode(&encoded, 100);

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_rle_encode_dense_cluster() {
        // Dense cluster: 100 checkboxes all the same color
        let mut data = vec![0u8; 1000];
        for i in 100..200 {
            data[i] = 5; // palette index 5
        }

        let encoded = rle_encode(&data);
        let decoded = rle_decode(&encoded, 1000);

        assert_eq!(decoded, data);
        // Should compress well: 3 runs (before, cluster, after)
        assert!(
            encoded.len() < 20,
            "Dense cluster should compress well, got {} bytes",
            encoded.len()
        );
    }

    #[test]
    fn test_rle_encode_alternating() {
        // Worst case: alternating values (no compression)
        let mut data = vec![0u8; 100];
        for i in 0..100 {
            data[i] = (i % 2) as u8;
        }

        let encoded = rle_encode(&data);
        let decoded = rle_decode(&encoded, 100);

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_rle_roundtrip_million() {
        // Full chunk with sparse data
        let mut data = vec![0u8; 1_000_000];
        // Add some scattered checkboxes
        data[0] = 1;
        data[999] = 2;
        data[500_000] = 3;
        data[999_999] = 4;

        let encoded = rle_encode(&data);
        let decoded = rle_decode(&encoded, 1_000_000);

        assert_eq!(decoded, data);
    }

    // === Palette Tests ===

    #[test]
    fn test_palette_empty() {
        let palette = Palette::new();
        assert_eq!(palette.len(), 0);
    }

    #[test]
    fn test_palette_add_color() {
        let mut palette = Palette::new();
        let idx = palette.get_or_add(255, 0, 0); // red
        assert_eq!(idx, 1); // 0 is reserved for unchecked

        let idx2 = palette.get_or_add(0, 255, 0); // green
        assert_eq!(idx2, 2);

        // Same color should return same index
        let idx3 = palette.get_or_add(255, 0, 0); // red again
        assert_eq!(idx3, 1);
    }

    #[test]
    fn test_palette_encode_decode() {
        let mut palette = Palette::new();
        palette.get_or_add(255, 0, 0);
        palette.get_or_add(0, 255, 0);
        palette.get_or_add(0, 0, 255);

        let encoded = palette.encode();
        let decoded = Palette::decode(&encoded);

        assert_eq!(decoded.get_color(1), Some((255, 0, 0)));
        assert_eq!(decoded.get_color(2), Some((0, 255, 0)));
        assert_eq!(decoded.get_color(3), Some((0, 0, 255)));
    }

    #[test]
    fn test_palette_max_colors() {
        let mut palette = Palette::new();

        // Add 255 unique colors (index 0 is reserved)
        for i in 1..=255u8 {
            let idx = palette.get_or_add(i, i, i);
            assert_eq!(idx, i);
        }

        // 256th color should fail or reuse closest
        // (implementation detail - test that it doesn't crash)
        let _ = palette.get_or_add(0, 0, 0);
    }
}

use crate::constants::{CELL_SIZE, CHUNK_SIZE};

/// Convert canvas coordinates to grid column/row (signed, infinite grid)
pub fn canvas_to_grid(
    mouse_x: f64,
    mouse_y: f64,
    offset_x: f64,
    offset_y: f64,
    scale: f64,
) -> (i32, i32) {
    let cell_size = CELL_SIZE * scale;
    let col = ((mouse_x - offset_x) / cell_size).floor() as i32;
    let row = ((mouse_y - offset_y) / cell_size).floor() as i32;
    (col, row)
}

/// Calculate chunk coordinates from global grid coordinates (signed)
pub fn grid_to_chunk_coords(col: i32, row: i32) -> (i32, i32) {
    // Use floor division for correct negative handling
    let chunk_x = col.div_euclid(CHUNK_SIZE as i32);
    let chunk_y = row.div_euclid(CHUNK_SIZE as i32);
    (chunk_x, chunk_y)
}

/// Calculate local coordinates within a chunk (always positive 0..CHUNK_SIZE)
pub fn grid_to_local(col: i32, row: i32) -> (u32, u32) {
    let local_x = col.rem_euclid(CHUNK_SIZE as i32) as u32;
    let local_y = row.rem_euclid(CHUNK_SIZE as i32) as u32;
    (local_x, local_y)
}

/// Encode chunk coordinates to a single i64 ID
pub fn chunk_coords_to_id(chunk_x: i32, chunk_y: i32) -> i64 {
    ((chunk_x as i64) << 32) | ((chunk_y as u32) as i64)
}

/// Decode chunk ID back to coordinates
pub fn chunk_id_to_coords(id: i64) -> (i32, i32) {
    let chunk_x = (id >> 32) as i32;
    let chunk_y = id as i32;
    (chunk_x, chunk_y)
}

/// Calculate visible chunk range with buffer (infinite grid, signed coords)
/// Returns (min_chunk_x, min_chunk_y, max_chunk_x, max_chunk_y)
pub fn visible_chunk_range(
    offset_x: f64,
    offset_y: f64,
    scale: f64,
    canvas_w: f64,
    canvas_h: f64,
) -> (i32, i32, i32, i32) {
    let cell_size = CELL_SIZE * scale;

    // Visible grid bounds (signed, can be negative)
    let min_col = ((-offset_x) / cell_size).floor() as i32;
    let min_row = ((-offset_y) / cell_size).floor() as i32;
    let max_col = ((canvas_w - offset_x) / cell_size).ceil() as i32;
    let max_row = ((canvas_h - offset_y) / cell_size).ceil() as i32;

    // Convert to chunk coordinates with 1-chunk buffer
    let (chunk_min_x, chunk_min_y) = grid_to_chunk_coords(min_col, min_row);
    let (chunk_max_x, chunk_max_y) = grid_to_chunk_coords(max_col, max_row);

    (
        chunk_min_x - 1,
        chunk_min_y - 1,
        chunk_max_x + 1,
        chunk_max_y + 1,
    )
}

/// Get set of chunk coordinates in visible range
pub fn visible_chunks(
    offset_x: f64,
    offset_y: f64,
    scale: f64,
    canvas_w: f64,
    canvas_h: f64,
) -> Vec<(i32, i32)> {
    let (min_cx, min_cy, max_cx, max_cy) =
        visible_chunk_range(offset_x, offset_y, scale, canvas_w, canvas_h);
    let mut chunks = Vec::new();
    for cy in min_cy..=max_cy {
        for cx in min_cx..=max_cx {
            chunks.push((cx, cy));
        }
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_to_chunk_coords_positive() {
        // Cell (500, 500) -> chunk (0, 0)
        assert_eq!(grid_to_chunk_coords(500, 500), (0, 0));
        // Cell (1000, 0) -> chunk (1, 0)
        assert_eq!(grid_to_chunk_coords(1000, 0), (1, 0));
        // Cell (2500, 3500) -> chunk (2, 3)
        assert_eq!(grid_to_chunk_coords(2500, 3500), (2, 3));
    }

    #[test]
    fn test_grid_to_chunk_coords_negative() {
        // Cell (-1, -1) -> chunk (-1, -1)
        assert_eq!(grid_to_chunk_coords(-1, -1), (-1, -1));
        // Cell (-1000, 0) -> chunk (-1, 0)
        assert_eq!(grid_to_chunk_coords(-1000, 0), (-1, 0));
        // Cell (-1001, 0) -> chunk (-2, 0)
        assert_eq!(grid_to_chunk_coords(-1001, 0), (-2, 0));
    }

    #[test]
    fn test_grid_to_local_positive() {
        assert_eq!(grid_to_local(500, 500), (500, 500));
        assert_eq!(grid_to_local(1500, 2500), (500, 500));
    }

    #[test]
    fn test_grid_to_local_negative() {
        // Cell (-1, -1) -> local (999, 999) within chunk (-1, -1)
        assert_eq!(grid_to_local(-1, -1), (999, 999));
        // Cell (-500, -500) -> local (500, 500) within chunk (-1, -1)
        assert_eq!(grid_to_local(-500, -500), (500, 500));
        // Cell (-1000, 0) -> local (0, 0) within chunk (-1, 0)
        assert_eq!(grid_to_local(-1000, 0), (0, 0));
    }

    #[test]
    fn test_chunk_coords_roundtrip() {
        let test_cases = [
            (0, 0),
            (1, 2),
            (-1, -1),
            (1000, -2000),
            (i32::MAX, i32::MIN),
        ];
        for (x, y) in test_cases {
            let id = chunk_coords_to_id(x, y);
            let (rx, ry) = chunk_id_to_coords(id);
            assert_eq!((rx, ry), (x, y), "Failed for ({}, {})", x, y);
        }
    }

    #[test]
    fn test_canvas_to_grid() {
        // At scale 1.0, cell size is 8.0 pixels
        // Click at (80, 80) with no offset -> grid (10, 10)
        assert_eq!(canvas_to_grid(80.0, 80.0, 0.0, 0.0, 1.0), (10, 10));

        // Click at (0, 0) with offset (80, 80) -> grid (-10, -10)
        assert_eq!(canvas_to_grid(0.0, 0.0, 80.0, 80.0, 1.0), (-10, -10));
    }
}

    #[test]
    fn test_chunk_id_zero() {
        let id = chunk_coords_to_id(0, 0);
        println!("chunk_coords_to_id(0, 0) = {}", id);
        assert_eq!(id, 0);
    }

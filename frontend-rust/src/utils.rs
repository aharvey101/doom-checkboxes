use crate::constants::{CELL_SIZE, GRID_HEIGHT, GRID_WIDTH};

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

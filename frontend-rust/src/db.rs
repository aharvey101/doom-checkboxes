//! Database integration module
//!
//! This module handles checkbox state management and integrates with the worker bridge.
//! It handles:
//! - Deserializing CheckboxChunk rows from BSATN
//! - Optimistic updates for immediate UI feedback
//! - Sending updates to worker for server synchronization

use crate::constants::CHUNK_DATA_SIZE;
use crate::state::AppState;
use crate::utils::{chunk_coords_to_id, grid_to_chunk_coords, grid_to_local};
use leptos::prelude::*;
use crate::constants::CHUNK_SIZE;

/// CheckboxChunk row structure matching the backend schema
#[derive(Debug, Clone)]
pub struct CheckboxChunk {
    pub chunk_id: i64,
    pub state: Vec<u8>,
    pub version: u64,
}

impl CheckboxChunk {
    /// Deserialize a CheckboxChunk from BSATN bytes
    pub fn from_bsatn(bytes: &[u8]) -> Option<Self> {
        // SpacetimeDB encodes product types as sequential fields
        // CheckboxChunk = { chunk_id: i64, state: Vec<u8>, version: u64 }
        let mut reader = bytes;

        // Read chunk_id (i64, little-endian)
        if reader.len() < 8 {
            return None;
        }
        let chunk_id = i64::from_le_bytes([
            reader[0], reader[1], reader[2], reader[3], reader[4], reader[5], reader[6], reader[7],
        ]);
        reader = &reader[8..];

        // Read state (Vec<u8>): length-prefixed with u32
        if reader.len() < 4 {
            return None;
        }
        let state_len = u32::from_le_bytes([reader[0], reader[1], reader[2], reader[3]]) as usize;
        reader = &reader[4..];

        if reader.len() < state_len {
            return None;
        }
        let state = reader[..state_len].to_vec();
        reader = &reader[state_len..];

        // Read version (u64, little-endian)
        if reader.len() < 8 {
            return None;
        }
        let version = u64::from_le_bytes([
            reader[0], reader[1], reader[2], reader[3], reader[4], reader[5], reader[6], reader[7],
        ]);

        Some(CheckboxChunk {
            chunk_id,
            state,
            version,
        })
    }
}

/// Get checkbox state at the given cell index
/// Returns (r, g, b, checked) or None if out of bounds
pub fn get_checkbox(data: &[u8], cell_index: usize) -> Option<(u8, u8, u8, bool)> {
    let byte_idx = cell_index * 4;
    if byte_idx + 3 < data.len() {
        let r = data[byte_idx];
        let g = data[byte_idx + 1];
        let b = data[byte_idx + 2];
        let checked = data[byte_idx + 3] != 0;
        Some((r, g, b, checked))
    } else {
        None
    }
}

/// Set checkbox state at the given cell index
pub fn set_checkbox(data: &mut [u8], cell_index: usize, r: u8, g: u8, b: u8, checked: bool) {
    let byte_idx = cell_index * 4;
    if byte_idx + 3 < data.len() {
        data[byte_idx] = r;
        data[byte_idx + 1] = g;
        data[byte_idx + 2] = b;
        data[byte_idx + 3] = if checked { 0xFF } else { 0x00 };
    }
}

/// Check if a checkbox is checked at the given cell index
pub fn is_checked(data: &[u8], cell_index: usize) -> bool {
    let byte_idx = cell_index * 4 + 3; // checked byte is at offset +3
    if byte_idx < data.len() {
        data[byte_idx] != 0
    } else {
        false
    }
}

/// Convert local coordinates to cell offset (for the new format)
pub fn local_to_cell_offset(local_col: u32, local_row: u32) -> u32 {
    local_row * CHUNK_SIZE + local_col
}

/// Toggle a checkbox at the given grid position (signed coords for infinite grid)
/// Returns the new checked state for immediate visual feedback
pub fn toggle_checkbox(state: AppState, col: i32, row: i32) -> Option<bool> {
    use crate::worker_bridge::send_to_worker;
    use crate::worker_protocol::MainToWorker;

    let (chunk_x, chunk_y) = grid_to_chunk_coords(col, row);
    let chunk_id = chunk_coords_to_id(chunk_x, chunk_y);
    let (local_col, local_row) = grid_to_local(col, row);
    let cell_offset = local_to_cell_offset(local_col, local_row) as usize;

    // Get user color
    let (r, g, b) = state.user_color.get_untracked();

    // Ensure chunk exists locally
    state.loaded_chunks.update(|chunks| {
        chunks
            .entry(chunk_id)
            .or_insert_with(|| vec![0u8; CHUNK_DATA_SIZE]);
    });

    // Get current value and toggle
    let current_value = state.loaded_chunks.with_untracked(|chunks| {
        chunks
            .get(&chunk_id)
            .map(|data| is_checked(data, cell_offset))
            .unwrap_or(false)
    });
    let new_value = !current_value;

    // Optimistic update - immediate UI feedback
    // (Server will reconcile when update comes back)
    state.loaded_chunks.update(|chunks| {
        if let Some(data) = chunks.get_mut(&chunk_id) {
            set_checkbox(data, cell_offset, r, g, b, new_value);
        }
    });

    // Send to worker (non-blocking - worker handles connection state)
    send_to_worker(MainToWorker::UpdateCheckbox {
        chunk_id,
        cell_offset: cell_offset as u32,
        r,
        g,
        b,
        checked: new_value,
    });

    Some(new_value)
}

/// Set a checkbox to checked at the given grid position (for drag-to-fill)
/// Returns true if the checkbox was changed (was unchecked), false if already checked
/// Note: This queues the update for batching instead of sending immediately
pub fn set_checkbox_checked(state: AppState, col: i32, row: i32) -> Option<bool> {
    let (chunk_x, chunk_y) = grid_to_chunk_coords(col, row);
    let chunk_id = chunk_coords_to_id(chunk_x, chunk_y);
    let (local_col, local_row) = grid_to_local(col, row);
    let cell_offset = local_to_cell_offset(local_col, local_row) as usize;

    // Get user color
    let (r, g, b) = state.user_color.get_untracked();

    // Ensure chunk exists locally (create empty chunk if needed)
    state.loaded_chunks.update(|chunks| {
        chunks
            .entry(chunk_id)
            .or_insert_with(|| vec![0u8; CHUNK_DATA_SIZE]);
    });

    // Get current value
    let current_value = state.loaded_chunks.with_untracked(|chunks| {
        chunks
            .get(&chunk_id)
            .map(|data| is_checked(data, cell_offset))
            .unwrap_or(false)
    });

    // Only update if not already checked
    if current_value {
        return Some(false); // Already checked, no change
    }

    // Optimistic update with user's color
    state.loaded_chunks.update(|chunks| {
        if let Some(data) = chunks.get_mut(&chunk_id) {
            set_checkbox(data, cell_offset, r, g, b, true);
        }
    });

    // Queue update for batching instead of sending immediately
    // PendingUpdate = (chunk_id, cell_offset, r, g, b, checked)
    state.pending_updates.update(|updates| {
        updates.push((chunk_id, cell_offset as u32, r, g, b, true));
    });

    Some(true) // Changed from unchecked to checked
}

/// Set a checkbox to unchecked at the given grid position (for eraser tool)
/// Returns true if the checkbox was changed (was checked), false if already unchecked
/// Note: This queues the update for batching instead of sending immediately
pub fn set_checkbox_unchecked(state: AppState, col: i32, row: i32) -> Option<bool> {
    let (chunk_x, chunk_y) = grid_to_chunk_coords(col, row);
    let chunk_id = chunk_coords_to_id(chunk_x, chunk_y);
    let (local_col, local_row) = grid_to_local(col, row);
    let cell_offset = local_to_cell_offset(local_col, local_row) as usize;

    // Get user color (even though we're unchecking, keep color for consistency)
    let (r, g, b) = state.user_color.get_untracked();

    // Ensure chunk exists locally (create empty chunk if needed)
    state.loaded_chunks.update(|chunks| {
        chunks
            .entry(chunk_id)
            .or_insert_with(|| vec![0u8; CHUNK_DATA_SIZE]);
    });

    // Get current value
    let current_value = state.loaded_chunks.with_untracked(|chunks| {
        chunks
            .get(&chunk_id)
            .map(|data| is_checked(data, cell_offset))
            .unwrap_or(false)
    });

    // Only update if currently checked
    if !current_value {
        return Some(false); // Already unchecked, no change
    }

    // Optimistic update - uncheck it
    state.loaded_chunks.update(|chunks| {
        if let Some(data) = chunks.get_mut(&chunk_id) {
            set_checkbox(data, cell_offset, r, g, b, false);
        }
    });

    // Queue update for batching
    // PendingUpdate = (chunk_id, cell_offset, r, g, b, checked)
    state.pending_updates.update(|updates| {
        updates.push((chunk_id, cell_offset as u32, r, g, b, false));
    });

    Some(true) // Changed from checked to unchecked
}

/// Flush pending updates to the server as a batch
/// This should be called on mouseup or after a debounce timer
pub fn flush_pending_updates(state: AppState) {
    use crate::worker_bridge::send_to_worker;
    use crate::worker_protocol::MainToWorker;

    // Take all pending updates atomically
    let updates = state.pending_updates.with_untracked(|u| u.clone());
    if updates.is_empty() {
        return;
    }

    // Clear pending updates
    state.pending_updates.set(Vec::new());

    web_sys::console::log_1(&format!("Flushing {} pending updates", updates.len()).into());

    // Send to worker
    send_to_worker(MainToWorker::BatchUpdate { updates });
}

/// Subscribe to a set of chunks by their coordinates
/// NOTE: In worker architecture, subscription is handled automatically by the worker
/// This function is kept for compatibility but does nothing
pub fn subscribe_to_chunks(_state: AppState, _chunks: Vec<(i32, i32)>) {
    // Worker handles all subscriptions automatically
    // This function exists for API compatibility only
}

//! Database integration module
//!
//! This module bridges the SpacetimeDB WebSocket client with Leptos reactive state.
//! It handles:
//! - Connection lifecycle
//! - Deserializing CheckboxChunk rows from BSATN
//! - Updating Leptos signals when data arrives
//! - Sending reducer calls for checkbox toggles

use crate::constants::CHUNK_DATA_SIZE;
use crate::state::{AppState, ConnectionStatus, PendingUpdate};
use crate::utils::{chunk_coords_to_id, grid_to_chunk_coords, grid_to_local};
use crate::ws_client::{call_reducer, connect, subscribe, SharedClient, SpacetimeClient};
use leptos::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

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

// Global client storage - we use thread_local for WASM safety
thread_local! {
    static CLIENT: RefCell<Option<SharedClient>> = const { RefCell::new(None) };
}

/// Store the client reference
fn set_client(client: SharedClient) {
    CLIENT.with(|c| {
        *c.borrow_mut() = Some(client);
    });
}

/// Get a clone of the client reference
fn get_client() -> Option<SharedClient> {
    CLIENT.with(|c| c.borrow().clone())
}

/// Initialize the database connection
pub fn init_connection(state: AppState) {
    let uri = get_spacetimedb_uri();
    let database = "checkboxes".to_string();

    web_sys::console::log_1(&format!("Connecting to SpacetimeDB at {}", uri).into());

    // Update status
    state
        .status_message
        .set(format!("Connecting to {}...", uri));

    // Create client
    let client = Rc::new(RefCell::new(SpacetimeClient::new()));

    // Store client for later use
    set_client(client.clone());

    // Set up callbacks
    {
        let mut client_mut = client.borrow_mut();

        // On connect callback
        let state_connect = state;
        client_mut.on_connect(Box::new(move |_identity, _token| {
            web_sys::console::log_1(&"Connected to SpacetimeDB, subscribing...".into());
            state_connect.status.set(ConnectionStatus::Connecting);
            state_connect
                .status_message
                .set("Subscribing...".to_string());

            // Subscribe to checkbox_chunk table
            if let Some(client) = get_client() {
                subscribe(&client, &["SELECT * FROM checkbox_chunk"]);
            }
        }));

        // On disconnect callback
        let state_disconnect = state;
        client_mut.on_disconnect(Box::new(move || {
            web_sys::console::log_1(&"Disconnected from SpacetimeDB".into());
            state_disconnect.status.set(ConnectionStatus::Error);
            state_disconnect
                .status_message
                .set("Disconnected".to_string());
        }));

        // On error callback
        let state_error = state;
        client_mut.on_error(Box::new(move |error| {
            web_sys::console::error_1(&format!("SpacetimeDB error: {}", error).into());
            state_error.status.set(ConnectionStatus::Error);
            state_error.status_message.set(format!("Error: {}", error));
        }));

        // On subscription applied
        let state_sub = state;
        client_mut.on_subscribe_applied(Box::new(move || {
            web_sys::console::log_1(&"Subscription applied".into());
            state_sub.status.set(ConnectionStatus::Connected);
            state_sub.status_message.set("Connected".to_string());
        }));

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
    }

    // Connect
    connect(client, &uri, &database);
}

/// Toggle a checkbox at the given grid position (signed coords for infinite grid)
/// Returns the new checked state for immediate visual feedback
pub fn toggle_checkbox(state: AppState, col: i32, row: i32) -> Option<bool> {
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

    // Get current value and toggle
    let current_value = state.loaded_chunks.with_untracked(|chunks| {
        chunks
            .get(&chunk_id)
            .map(|data| is_checked(data, cell_offset))
            .unwrap_or(false)
    });
    let new_value = !current_value;

    // Optimistic update - set with user's color
    state.loaded_chunks.update(|chunks| {
        if let Some(data) = chunks.get_mut(&chunk_id) {
            set_checkbox(data, cell_offset, r, g, b, new_value);
        }
    });

    // Send to server
    if let Some(client) = get_client() {
        // Encode reducer arguments: (chunk_id: i64, cell_offset: u32, r: u8, g: u8, b: u8, checked: bool)
        let args = encode_update_checkbox_args(chunk_id, cell_offset as u32, r, g, b, new_value);
        call_reducer(&client, "update_checkbox", &args);
    }

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
    // Take all pending updates atomically
    let updates = state.pending_updates.with_untracked(|u| u.clone());
    if updates.is_empty() {
        return;
    }

    // Clear pending updates
    state.pending_updates.set(Vec::new());

    web_sys::console::log_1(&format!("Flushing {} pending updates", updates.len()).into());

    // Send batch to server
    if let Some(client) = get_client() {
        let args = encode_batch_update_args(&updates);
        call_reducer(&client, "batch_update_checkboxes", &args);
    }
}

/// Encode arguments for batch_update_checkboxes reducer
/// Format: length-prefixed array of CheckboxUpdate { chunk_id: i64, cell_offset: u32, r: u8, g: u8, b: u8, checked: bool }
fn encode_batch_update_args(updates: &[PendingUpdate]) -> Vec<u8> {
    // BSATN encoding for Vec<CheckboxUpdate>
    // Vec is encoded as: length (u32) followed by elements
    // Each element is encoded as: i64 + u32 + u8 + u8 + u8 + bool = 16 bytes
    let mut buf = Vec::with_capacity(4 + updates.len() * 16);

    // Array length (u32, little-endian)
    buf.extend_from_slice(&(updates.len() as u32).to_le_bytes());

    // Each update: (chunk_id, cell_offset, r, g, b, checked)
    for (chunk_id, cell_offset, r, g, b, checked) in updates {
        buf.extend_from_slice(&chunk_id.to_le_bytes());
        buf.extend_from_slice(&cell_offset.to_le_bytes());
        buf.push(*r);
        buf.push(*g);
        buf.push(*b);
        buf.push(if *checked { 1 } else { 0 });
    }

    buf
}

/// Encode arguments for update_checkbox reducer
/// Format: CheckboxUpdate { chunk_id: i64, cell_offset: u32, r: u8, g: u8, b: u8, checked: bool }
fn encode_update_checkbox_args(
    chunk_id: i64,
    cell_offset: u32,
    r: u8,
    g: u8,
    b: u8,
    checked: bool,
) -> Vec<u8> {
    // BSATN encoding: product type with six fields
    // i64 + u32 + u8 + u8 + u8 + bool = 16 bytes
    let mut buf = Vec::with_capacity(16);

    // chunk_id: i64 (little-endian)
    buf.extend_from_slice(&chunk_id.to_le_bytes());

    // cell_offset: u32 (little-endian)
    buf.extend_from_slice(&cell_offset.to_le_bytes());

    // r: u8
    buf.push(r);

    // g: u8
    buf.push(g);

    // b: u8
    buf.push(b);

    // checked: bool (1 byte)
    buf.push(if checked { 1 } else { 0 });

    buf
}

/// Subscribe to a set of chunks by their coordinates
pub fn subscribe_to_chunks(state: AppState, chunks: Vec<(i32, i32)>) {
    if let Some(client) = get_client() {
        for (chunk_x, chunk_y) in chunks {
            let chunk_id = chunk_coords_to_id(chunk_x, chunk_y);

            // Skip if already subscribed or loading
            if state
                .subscribed_chunks
                .with_untracked(|s| s.contains(&chunk_id))
            {
                continue;
            }
            if state
                .loading_chunks
                .with_untracked(|l| l.contains(&chunk_id))
            {
                continue;
            }

            // Mark as loading
            state.loading_chunks.update(|loading| {
                loading.insert(chunk_id);
            });

            // Subscribe via SpacetimeDB
            let query = format!("SELECT * FROM checkbox_chunk WHERE chunk_id = {}", chunk_id);
            subscribe(&client, &[&query]);

            web_sys::console::log_1(
                &format!(
                    "Subscribing to chunk ({}, {}) id={}",
                    chunk_x, chunk_y, chunk_id
                )
                .into(),
            );
        }
    }
}

/// Get the SpacetimeDB URI based on environment
fn get_spacetimedb_uri() -> String {
    // Check if we're running locally
    let window = web_sys::window().expect("no window");
    let location = window.location();
    let hostname = location.hostname().unwrap_or_default();

    if hostname == "localhost" || hostname == "127.0.0.1" {
        "ws://127.0.0.1:3000".to_string()
    } else {
        "wss://maincloud.spacetimedb.com".to_string()
    }
}

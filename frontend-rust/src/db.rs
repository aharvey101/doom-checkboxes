//! Database integration module
//!
//! This module bridges the SpacetimeDB WebSocket client with Leptos reactive state.
//! It handles:
//! - Connection lifecycle
//! - Deserializing CheckboxChunk rows from BSATN
//! - Updating Leptos signals when data arrives
//! - Sending reducer calls for checkbox toggles

use crate::constants::CHUNK_DATA_SIZE;
use crate::state::{AppState, ConnectionStatus};
use crate::utils::{grid_to_chunk_id, grid_to_local, local_to_bit_offset};
use crate::ws_client::{call_reducer, connect, subscribe, SharedClient, SpacetimeClient};
use leptos::prelude::*;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// CheckboxChunk row structure matching the backend schema
#[derive(Debug, Clone)]
pub struct CheckboxChunk {
    pub chunk_id: u32,
    pub state: Vec<u8>,
    pub version: u64,
}

impl CheckboxChunk {
    /// Deserialize a CheckboxChunk from BSATN bytes
    pub fn from_bsatn(bytes: &[u8]) -> Option<Self> {
        // SpacetimeDB encodes product types as sequential fields
        // CheckboxChunk = { chunk_id: u32, state: Vec<u8>, version: u64 }
        let mut reader = bytes;

        // Read chunk_id (u32, little-endian)
        if reader.len() < 4 {
            return None;
        }
        let chunk_id = u32::from_le_bytes([reader[0], reader[1], reader[2], reader[3]]);
        reader = &reader[4..];

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

/// Get a bit value at the given index
pub fn get_bit(data: &[u8], bit_index: usize) -> bool {
    let byte_idx = bit_index / 8;
    let bit_idx = bit_index % 8;
    if byte_idx < data.len() {
        (data[byte_idx] >> bit_idx) & 1 == 1
    } else {
        false
    }
}

/// Set a bit value at the given index
pub fn set_bit(data: &mut [u8], bit_index: usize, value: bool) {
    let byte_idx = bit_index / 8;
    let bit_idx = bit_index % 8;
    if byte_idx < data.len() {
        if value {
            data[byte_idx] |= 1 << bit_idx;
        } else {
            data[byte_idx] &= !(1 << bit_idx);
        }
    }
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

/// Toggle a checkbox at the given grid position
/// Returns the new checked state for immediate visual feedback
pub fn toggle_checkbox(state: AppState, col: u32, row: u32) -> Option<bool> {
    let chunk_id = grid_to_chunk_id(col, row);
    let (local_col, local_row) = grid_to_local(col, row);
    let bit_offset = local_to_bit_offset(local_col, local_row) as usize;

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
            .map(|data| get_bit(data, bit_offset))
            .unwrap_or(false)
    });
    let new_value = !current_value;

    // Optimistic update
    state.loaded_chunks.update(|chunks| {
        if let Some(data) = chunks.get_mut(&chunk_id) {
            set_bit(data, bit_offset, new_value);
        }
    });

    // Send to server
    if let Some(client) = get_client() {
        // Encode reducer arguments: (chunk_id: u32, bit_offset: u32, checked: bool)
        let args = encode_update_checkbox_args(chunk_id, bit_offset as u32, new_value);
        call_reducer(&client, "update_checkbox", &args);
    }

    Some(new_value)
}

/// Set a checkbox to checked at the given grid position (for drag-to-fill)
/// Returns true if the checkbox was changed (was unchecked), false if already checked
pub fn set_checkbox_checked(state: AppState, col: u32, row: u32) -> Option<bool> {
    let chunk_id = grid_to_chunk_id(col, row);
    let (local_col, local_row) = grid_to_local(col, row);
    let bit_offset = local_to_bit_offset(local_col, local_row) as usize;

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
            .map(|data| get_bit(data, bit_offset))
            .unwrap_or(false)
    });

    // Only update if not already checked
    if current_value {
        return Some(false); // Already checked, no change
    }

    // Optimistic update
    state.loaded_chunks.update(|chunks| {
        if let Some(data) = chunks.get_mut(&chunk_id) {
            set_bit(data, bit_offset, true);
        }
    });

    // Send to server
    if let Some(client) = get_client() {
        let args = encode_update_checkbox_args(chunk_id, bit_offset as u32, true);
        call_reducer(&client, "update_checkbox", &args);
    }

    Some(true) // Changed from unchecked to checked
}

/// Encode arguments for update_checkbox reducer
fn encode_update_checkbox_args(chunk_id: u32, bit_offset: u32, checked: bool) -> Vec<u8> {
    // BSATN encoding: product type with three fields
    // u32 + u32 + bool
    let mut buf = Vec::with_capacity(9);

    // chunk_id: u32 (little-endian)
    buf.extend_from_slice(&chunk_id.to_le_bytes());

    // bit_offset: u32 (little-endian)
    buf.extend_from_slice(&bit_offset.to_le_bytes());

    // checked: bool (1 byte)
    buf.push(if checked { 1 } else { 0 });

    buf
}

/// Subscribe to a set of chunks
pub fn subscribe_to_chunks(state: AppState, chunk_ids: HashSet<u32>) {
    if let Some(client) = get_client() {
        for chunk_id in chunk_ids {
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

            web_sys::console::log_1(&format!("Subscribing to chunk {}", chunk_id).into());
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

//! Database integration module
//!
//! This module bridges the SpacetimeDB WebSocket client with Leptos reactive state.
//! It handles:
//! - Connection lifecycle
//! - Deserializing CheckboxChunk rows from BSATN
//! - Updating Leptos signals when data arrives
//! - Sending reducer calls for checkbox toggles

use crate::constants::{GRID_WIDTH, TOTAL_CHECKBOXES};
use crate::state::{AppState, ConnectionStatus};
use crate::ws_client::{call_reducer, connect, subscribe, SharedClient, SpacetimeClient};
use leptos::prelude::*;
use std::cell::RefCell;
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

/// Count checked bits in the chunk data
pub fn count_checked(data: &[u8]) -> u32 {
    data.iter().map(|b| b.count_ones()).sum()
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

        // On chunk insert (also handles initial data from subscription)
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

                if chunk.chunk_id == 0 {
                    let checked = count_checked(&chunk.state);
                    state_insert.chunk_data.set(chunk.state);
                    state_insert.checked_count.set(checked);
                }
            }
        }));

        // On chunk update (state changes from other clients)
        let state_update = state;
        client_mut.on_chunk_update(Box::new(move |_old_bytes: &[u8], new_bytes: &[u8]| {
            if let Some(chunk) = CheckboxChunk::from_bsatn(new_bytes) {
                if chunk.chunk_id == 0 {
                    // Only update if data actually changed (skip our own optimistic updates)
                    let current = state_update.chunk_data.get_untracked();
                    if current != chunk.state {
                        web_sys::console::log_1(
                            &format!(
                                "Chunk {} updated from server, version {}",
                                chunk.chunk_id, chunk.version
                            )
                            .into(),
                        );
                        let checked = count_checked(&chunk.state);
                        state_update.chunk_data.set(chunk.state);
                        state_update.checked_count.set(checked);
                    }
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
    let bit_index = (row * GRID_WIDTH as u32 + col) as usize;

    if bit_index >= TOTAL_CHECKBOXES as usize {
        return None;
    }

    // Get current state and toggle
    let current_state = state.chunk_data.get_untracked();
    let current_value = get_bit(&current_state, bit_index);
    let new_value = !current_value;

    // Optimistic update
    state.chunk_data.update(|data| {
        set_bit(data, bit_index, new_value);
    });
    state.checked_count.update(|count| {
        if new_value {
            *count += 1;
        } else {
            *count = count.saturating_sub(1);
        }
    });

    // Send to server
    if let Some(client) = get_client() {
        // Encode reducer arguments: (chunk_id: u32, bit_offset: u32, checked: bool)
        let args = encode_update_checkbox_args(0, bit_index as u32, new_value);
        call_reducer(&client, "update_checkbox", &args);
    }

    Some(new_value)
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

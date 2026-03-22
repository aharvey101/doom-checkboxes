//! Worker-side SpacetimeDB client
//!
//! Handles WebSocket connection, BSATN encoding, and reconnection logic
//!
//! Note: Helper functions like `send_to_main_thread`, `handle_ws_message`, and
//! `handle_ws_close` are defined later in this file.

use super::protocol::WorkerToMain;
use bytes::Bytes;
use spacetimedb_client_api_messages::websocket::{
    common::QuerySetId,
    v2::{
        CallReducer, CallReducerFlags, ClientMessage, InitialConnection, QueryRows,
        ReducerResult, ServerMessage, Subscribe, SubscribeApplied, SubscriptionError,
        TableUpdate, TableUpdateRows, TransactionUpdate, Unsubscribe, UnsubscribeFlags,
    },
};
use spacetimedb_lib::bsatn;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{DedicatedWorkerGlobalScope, WebSocket};

// Reconnection constants
const BACKOFF_SCHEDULE: [u32; 5] = [5000, 10000, 20000, 40000, 60000];
const MAX_RETRIES: u32 = 5;

// Compression tags for server messages
const COMPRESSION_NONE: u8 = 0;
const COMPRESSION_BROTLI: u8 = 1;
const COMPRESSION_GZIP: u8 = 2;

// WebSocket protocol identifier
const WS_PROTOCOL: &str = "v2.bsatn.spacetimedb";

/// SpacetimeDB client state
pub struct WorkerClient {
    ws: Option<WebSocket>,
    uri: String,
    database: String,
    reconnect_attempt: u32,
    intentional_disconnect: bool,
    subscribed_chunks: Vec<i64>,
    request_id: u32,
    chunk_subscription_id: Option<QuerySetId>,
    // Store closures to prevent memory leaks
    onopen_cb: Option<Closure<dyn FnMut(web_sys::Event)>>,
    onmessage_cb: Option<Closure<dyn FnMut(web_sys::MessageEvent)>>,
    onerror_cb: Option<Closure<dyn FnMut(web_sys::Event)>>,
    onclose_cb: Option<Closure<dyn FnMut(web_sys::CloseEvent)>>,
}

thread_local! {
    static CLIENT: RefCell<Option<Rc<RefCell<WorkerClient>>>> = const { RefCell::new(None) };
}

impl WorkerClient {
    pub fn new() -> Self {
        Self {
            ws: None,
            uri: String::new(),
            database: String::new(),
            reconnect_attempt: 0,
            intentional_disconnect: false,
            subscribed_chunks: Vec::new(),
            request_id: 0,
            chunk_subscription_id: None,
            onopen_cb: None,
            onmessage_cb: None,
            onerror_cb: None,
            onclose_cb: None,
        }
    }

    /// Get the next request ID
    fn next_request_id(&mut self) -> u32 {
        self.request_id += 1;
        self.request_id
    }

    /// Connect to SpacetimeDB
    pub fn connect(&mut self, uri: String, database: String) {
        web_sys::console::log_1(&format!("Connecting to {} / {}", uri, database).into());

        self.uri = uri.clone();
        self.database = database.clone();
        self.intentional_disconnect = false;

        // Clean up old WebSocket and closures
        if let Some(old_ws) = self.ws.take() {
            old_ws.close().ok();
        }
        self.onopen_cb = None;
        self.onmessage_cb = None;
        self.onerror_cb = None;
        self.onclose_cb = None;

        let url = format!("{}/v1/database/{}/subscribe", uri, database);

        let ws = WebSocket::new_with_str(&url, WS_PROTOCOL).expect("Failed to create WebSocket");
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        // Set up WebSocket callbacks
        let onopen = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            web_sys::console::log_1(&"WebSocket connected".into());

            // Reset reconnection counter on successful connection
            CLIENT.with(|c| {
                if let Some(client) = c.borrow().as_ref() {
                    client.borrow_mut().reconnect_attempt = 0;
                }
            });

            send_to_main_thread(WorkerToMain::Connected);
        }) as Box<dyn FnMut(_)>);

        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        self.onopen_cb = Some(onopen);

        let onmessage = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
            handle_ws_message(event);
        }) as Box<dyn FnMut(_)>);

        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        self.onmessage_cb = Some(onmessage);

        let onerror = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            web_sys::console::error_1(&"WebSocket error".into());
        }) as Box<dyn FnMut(_)>);

        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        self.onerror_cb = Some(onerror);

        let onclose = Closure::wrap(Box::new(move |_event: web_sys::CloseEvent| {
            web_sys::console::log_1(&"WebSocket closed".into());
            handle_ws_close();
        }) as Box<dyn FnMut(_)>);

        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        self.onclose_cb = Some(onclose);

        self.ws = Some(ws);
    }

    /// Subscribe to checkbox_chunk (initial load) and checkbox_delta (live updates).
    /// After the initial chunk data arrives, we unsubscribe from checkbox_chunk
    /// to avoid receiving 4MB blobs on every snapshot.
    pub fn subscribe(&mut self) {
        // Subscribe to full chunk state for initial load
        let request_id = self.next_request_id();
        let chunk_query_set_id = QuerySetId::new(request_id);
        self.chunk_subscription_id = Some(chunk_query_set_id);
        let subscribe_chunks = Subscribe {
            request_id,
            query_set_id: chunk_query_set_id,
            query_strings: vec!["SELECT * FROM checkbox_chunk".into()].into_boxed_slice(),
        };
        self.send_message(&ClientMessage::Subscribe(subscribe_chunks));

        // Subscribe to packed frame table for real-time updates
        let request_id2 = self.next_request_id();
        let subscribe_frames = Subscribe {
            request_id: request_id2,
            query_set_id: QuerySetId::new(request_id2),
            query_strings: vec!["SELECT * FROM checkbox_frame".into()].into_boxed_slice(),
        };
        self.send_message(&ClientMessage::Subscribe(subscribe_frames));
    }

    /// Unsubscribe from checkbox_chunk after initial data is loaded
    pub fn unsubscribe_chunks(&mut self) {
        if let Some(query_set_id) = self.chunk_subscription_id.take() {
            let request_id = self.next_request_id();
            let unsub = Unsubscribe {
                request_id,
                query_set_id,
                flags: UnsubscribeFlags::default(),
            };
            self.send_message(&ClientMessage::Unsubscribe(unsub));
            web_sys::console::log_1(&"[worker] Unsubscribed from checkbox_chunk (initial load complete)".into());
        }
    }

    /// Send BSATN-encoded reducer call
    pub fn call_reducer(&mut self, reducer_name: &str, args: &[u8]) -> u32 {
        let request_id = self.next_request_id();

        let call = CallReducer {
            request_id,
            flags: CallReducerFlags::Default,
            reducer: reducer_name.into(),
            args: Bytes::copy_from_slice(args),
        };

        let message = ClientMessage::CallReducer(call);
        self.send_message(&message);

        request_id
    }

    /// Send a client message
    fn send_message(&self, message: &ClientMessage) {
        let Some(ws) = &self.ws else {
            web_sys::console::error_1(&"WebSocket not connected".into());
            return;
        };

        let bytes = match bsatn::to_vec(message) {
            Ok(b) => b,
            Err(e) => {
                web_sys::console::error_1(&format!("Failed to serialize message: {:?}", e).into());
                return;
            }
        };

        if let Err(e) = ws.send_with_u8_array(&bytes) {
            web_sys::console::error_1(&format!("Failed to send message: {:?}", e).into());
        }
    }

    /// Disconnect
    pub fn disconnect(&mut self) {
        self.intentional_disconnect = true;
        if let Some(ws) = self.ws.take() {
            ws.close().ok();
        }
    }

    /// Handle reconnection
    pub fn reconnect(&mut self) {
        if self.intentional_disconnect {
            return;
        }

        if self.reconnect_attempt >= MAX_RETRIES {
            send_to_main_thread(WorkerToMain::FatalError {
                message: format!("Connection lost after {} retries", MAX_RETRIES),
            });
            return;
        }

        let backoff_ms = BACKOFF_SCHEDULE
            .get(self.reconnect_attempt as usize)
            .copied()
            .unwrap_or(*BACKOFF_SCHEDULE.last().unwrap());
        web_sys::console::log_1(
            &format!(
                "Reconnecting in {}ms (attempt {}/{})",
                backoff_ms,
                self.reconnect_attempt + 1,
                MAX_RETRIES
            )
            .into(),
        );

        self.reconnect_attempt += 1;

        // Schedule reconnection
        let window = js_sys::global();
        let closure = Closure::once(Box::new(move || {
            CLIENT.with(|c| {
                if let Some(client) = c.borrow().as_ref() {
                    let mut client_mut = client.borrow_mut();
                    let uri = client_mut.uri.clone();
                    let database = client_mut.database.clone();
                    client_mut.connect(uri, database);
                }
            });
        }) as Box<dyn FnOnce()>);

        let scope = window
            .dyn_into::<DedicatedWorkerGlobalScope>()
            .expect("not in worker");
        scope
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                backoff_ms as i32,
            )
            .ok();
        closure.forget();
    }
}

/// Initialize global client
pub fn init_client() {
    CLIENT.with(|c| {
        *c.borrow_mut() = Some(Rc::new(RefCell::new(WorkerClient::new())));
    });
}

/// Get client reference
pub fn with_client<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut WorkerClient) -> R,
{
    CLIENT.with(|c| c.borrow().as_ref().map(|client| f(&mut client.borrow_mut())))
}

/// Handle WebSocket message
fn handle_ws_message(event: web_sys::MessageEvent) {
    let data = event.data();

    // Check if it's a string first (doesn't consume data)
    if let Some(text) = data.as_string() {
        // JSON message (for subscribe acknowledgment)
        web_sys::console::log_1(&format!("JSON message: {}", text).into());
    } else if let Ok(array_buffer) = data.dyn_into::<js_sys::ArrayBuffer>() {
        // Binary BSATN message
        let uint8_array = js_sys::Uint8Array::new(&array_buffer);
        let bytes = uint8_array.to_vec();

        // Parse SpacetimeDB message
        parse_spacetimedb_message(&bytes);
    }
}

/// Parse SpacetimeDB binary message using v2 protocol
///
/// Protocol:
/// 1. First byte is compression tag (0=none, 1=brotli, 2=gzip)
/// 2. Decompress the message bytes
/// 3. Deserialize using bsatn::from_slice into ServerMessage
/// 4. Handle ServerMessage variants
fn parse_spacetimedb_message(bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }

    let t0 = js_sys::Date::now();

    // First byte is compression tag
    let compression_tag = bytes[0];
    let message_bytes = &bytes[1..];

    // Decompress if needed
    let decompressed = match compression_tag {
        COMPRESSION_NONE => message_bytes.to_vec(),
        COMPRESSION_BROTLI => match decompress_brotli(message_bytes) {
            Ok(data) => data,
            Err(e) => {
                web_sys::console::error_1(&format!("Brotli decompression failed: {}", e).into());
                return;
            }
        },
        COMPRESSION_GZIP => match decompress_gzip(message_bytes) {
            Ok(data) => data,
            Err(e) => {
                web_sys::console::error_1(&format!("Gzip decompression failed: {}", e).into());
                return;
            }
        },
        _ => {
            web_sys::console::error_1(
                &format!("Unknown compression tag: {}", compression_tag).into(),
            );
            return;
        }
    };

    let t1 = js_sys::Date::now();

    // Deserialize the server message
    let message: ServerMessage = match bsatn::from_slice(&decompressed) {
        Ok(msg) => msg,
        Err(e) => {
            web_sys::console::error_1(&format!("Failed to deserialize message: {:?}", e).into());
            return;
        }
    };

    let t2 = js_sys::Date::now();

    // Log timing for chunk-sized messages
    let compressed_kb = bytes.len() / 1024;
    let decompressed_kb = decompressed.len() / 1024;
    if decompressed_kb > 100 {
        web_sys::console::log_1(&format!(
            "[PERF worker] decompress={:.0}ms bsatn_parse={:.0}ms | {}KB -> {}KB",
            t1 - t0, t2 - t1, compressed_kb, decompressed_kb
        ).into());
    }

    // Handle the message
    handle_server_message(message);
}

/// Decompress brotli data
fn decompress_brotli(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut reader = brotli::Decompressor::new(data, 4096);
    std::io::copy(&mut reader, &mut output).map_err(|e| e.to_string())?;
    Ok(output)
}

/// Decompress gzip data
fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, String> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut decoder = GzDecoder::new(data);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|e| e.to_string())?;
    Ok(output)
}

/// Handle a deserialized server message
fn handle_server_message(message: ServerMessage) {
    match message {
        ServerMessage::InitialConnection(init) => {
            handle_initial_connection(init);
        }
        ServerMessage::SubscribeApplied(sub) => {
            handle_subscribe_applied(sub);
        }
        ServerMessage::TransactionUpdate(tx) => {
            handle_transaction_update(tx);
        }
        ServerMessage::SubscriptionError(err) => {
            handle_subscription_error(err);
        }
        ServerMessage::ReducerResult(result) => {
            handle_reducer_result(result);
        }
        ServerMessage::UnsubscribeApplied(_) => {
            web_sys::console::log_1(&"Unsubscribe applied".into());
        }
        ServerMessage::OneOffQueryResult(_) => {
            web_sys::console::log_1(&"One-off query result".into());
        }
        ServerMessage::ProcedureResult(_) => {
            web_sys::console::log_1(&"Procedure result".into());
        }
    }
}

/// Handle initial connection message
fn handle_initial_connection(init: InitialConnection) {
    web_sys::console::log_1(&format!("Connected with identity: {:?}", init.identity).into());
    send_to_main_thread(WorkerToMain::Connected);
}

/// Handle subscribe applied message
fn handle_subscribe_applied(sub: SubscribeApplied) {
    web_sys::console::log_1(
        &format!("Subscribe applied for query set {:?}", sub.query_set_id).into(),
    );

    // Process initial rows
    process_query_rows(&sub.rows);

    // If this was the chunk subscription, unsubscribe now that initial data is loaded.
    // We only need deltas for live updates going forward.
    with_client(|client| {
        if client.chunk_subscription_id == Some(sub.query_set_id) {
            client.unsubscribe_chunks();
        }
    });
}

/// Handle transaction update message
fn handle_transaction_update(tx: TransactionUpdate) {
    for query_set in tx.query_sets.iter() {
        for table in query_set.tables.iter() {
            process_table_update(table);
        }
    }
}

/// Handle subscription error
fn handle_subscription_error(err: SubscriptionError) {
    let error_msg = format!("Subscription error: {}", err.error);
    web_sys::console::error_1(&error_msg.clone().into());
}

/// Handle reducer result
fn handle_reducer_result(result: ReducerResult) {
    web_sys::console::log_1(
        &format!("Reducer result: request_id={} success={:?}",
                 result.request_id, result.result).into()
    );
}

/// Process query rows (initial subscription data)
fn process_query_rows(rows: &QueryRows) {
    for table_rows in rows.tables.iter() {
        let table_name: &str = &table_rows.table;

        if table_name == "checkbox_chunk" {
            for row_bytes in &table_rows.rows {
                if let Some(chunk) = parse_checkbox_chunk(&row_bytes) {
                    send_to_main_thread(WorkerToMain::ChunkInserted {
                        chunk_id: chunk.chunk_id,
                        state: chunk.state,
                        version: chunk.version,
                    });
                }
            }
        }
        // Ignore initial checkbox_delta rows — we only care about live inserts
    }
}

/// Process a table update
fn process_table_update(table: &TableUpdate) {
    let table_name: &str = &table.table_name;

    if table_name == "checkbox_chunk" {
        process_chunk_table_update(table);
    } else if table_name == "checkbox_delta" {
        process_delta_table_update(table);
    } else if table_name == "checkbox_frame" {
        process_frame_table_update(table);
    }
}

/// Process checkbox_chunk table updates (full chunk state)
fn process_chunk_table_update(table: &TableUpdate) {
    for rows in table.rows.iter() {
        match rows {
            TableUpdateRows::PersistentTable(persistent) => {
                for row_bytes in &persistent.inserts {
                    if let Some(chunk) = parse_checkbox_chunk(&row_bytes) {
                        send_to_main_thread(WorkerToMain::ChunkInserted {
                            chunk_id: chunk.chunk_id,
                            state: chunk.state,
                            version: chunk.version,
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

/// Process checkbox_delta table updates — send as lightweight binary to main thread.
/// Binary format: [tag: u8 = 3] [N × 16 bytes: chunk_id(8) + cell_offset(4) + r + g + b + checked]
fn process_delta_table_update(table: &TableUpdate) {
    let mut deltas: Vec<u8> = Vec::new();
    let mut count = 0u32;

    for rows in table.rows.iter() {
        match rows {
            TableUpdateRows::PersistentTable(persistent) => {
                for row_bytes in &persistent.inserts {
                    if let Some(delta) = parse_checkbox_delta(&row_bytes) {
                        deltas.extend_from_slice(&delta.chunk_id.to_le_bytes());
                        deltas.extend_from_slice(&delta.cell_offset.to_le_bytes());
                        deltas.push(delta.r);
                        deltas.push(delta.g);
                        deltas.push(delta.b);
                        deltas.push(if delta.checked { 1 } else { 0 });
                        count += 1;
                    }
                }
            }
            _ => {}
        }
    }

    if count == 0 {
        return;
    }

    // Send as binary: [tag=3] [count: u32] [deltas...]
    let scope = js_sys::global()
        .dyn_into::<DedicatedWorkerGlobalScope>()
        .expect("not in worker");

    let total_len = 1 + 4 + deltas.len();
    let buffer = js_sys::ArrayBuffer::new(total_len as u32);
    let view = js_sys::Uint8Array::new(&buffer);

    let mut header = [0u8; 5];
    header[0] = 3; // tag for DeltaBatch
    header[1..5].copy_from_slice(&count.to_le_bytes());
    view.set(&js_sys::Uint8Array::from(&header[..]), 0);

    let delta_view = unsafe { js_sys::Uint8Array::view(&deltas) };
    view.set(&delta_view, 5);

    let transfer = js_sys::Array::new();
    transfer.push(&buffer);
    scope.post_message_with_transfer(&buffer, &transfer).expect("postMessage failed");

    if count > 100 {
        web_sys::console::log_1(&format!(
            "[PERF worker->main] delta_batch {} deltas ({}KB)",
            count, deltas.len() / 1024
        ).into());
    }
}

/// Process checkbox_frame table updates — forward packed binary directly to main thread.
/// The frame's `data` field is already packed as [N × 16 bytes] in the format
/// the main thread expects, so no per-pixel parsing is needed.
fn process_frame_table_update(table: &TableUpdate) {
    let scope = js_sys::global()
        .dyn_into::<DedicatedWorkerGlobalScope>()
        .expect("not in worker");

    for rows in table.rows.iter() {
        match rows {
            TableUpdateRows::PersistentTable(persistent) => {
                for row_bytes in &persistent.inserts {
                    if let Some(frame_data) = parse_checkbox_frame(&row_bytes) {
                        if frame_data.is_empty() {
                            continue;
                        }
                        let count = (frame_data.len() / 16) as u32;

                        // Send as binary: [tag=3] [count: u32] [data...]
                        let total_len = 1 + 4 + frame_data.len();
                        let buffer = js_sys::ArrayBuffer::new(total_len as u32);
                        let view = js_sys::Uint8Array::new(&buffer);

                        let mut header = [0u8; 5];
                        header[0] = 3; // DeltaBatch tag
                        header[1..5].copy_from_slice(&count.to_le_bytes());
                        view.set(&js_sys::Uint8Array::from(&header[..]), 0);

                        let data_view = unsafe { js_sys::Uint8Array::view(&frame_data) };
                        view.set(&data_view, 5);

                        let transfer = js_sys::Array::new();
                        transfer.push(&buffer);
                        scope.post_message_with_transfer(&buffer, &transfer).expect("postMessage failed");

                        if count > 100 {
                            web_sys::console::log_1(&format!(
                                "[PERF worker->main] frame {} updates ({}KB packed)",
                                count, frame_data.len() / 1024
                            ).into());
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Parse a CheckboxFrame from BSATN — extract just the data field.
/// BSATN layout: seq(u64) + data(length-prefixed Vec<u8>)
fn parse_checkbox_frame(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 12 { // 8 (seq) + 4 (data len)
        return None;
    }
    // Skip seq (8 bytes)
    let reader = &bytes[8..];
    // Read data Vec<u8>: length-prefixed with u32
    let data_len = u32::from_le_bytes(reader[..4].try_into().ok()?) as usize;
    let reader = &reader[4..];
    if reader.len() < data_len {
        return None;
    }
    Some(reader[..data_len].to_vec())
}

/// Parse a single CheckboxChunk from BSATN
fn parse_checkbox_chunk(bytes: &[u8]) -> Option<CheckboxChunk> {
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

struct CheckboxChunk {
    chunk_id: i64,
    state: Vec<u8>,
    version: u64,
}

/// Parse a single CheckboxDelta from BSATN
/// Fields: seq(u64) + chunk_id(i64) + cell_offset(u32) + r(u8) + g(u8) + b(u8) + checked(bool)
fn parse_checkbox_delta(bytes: &[u8]) -> Option<CheckboxDelta> {
    if bytes.len() < 8 + 8 + 4 + 1 + 1 + 1 + 1 {
        return None;
    }
    let mut reader = bytes;

    // seq: u64
    let seq = u64::from_le_bytes(reader[..8].try_into().ok()?);
    reader = &reader[8..];

    // chunk_id: i64
    let chunk_id = i64::from_le_bytes(reader[..8].try_into().ok()?);
    reader = &reader[8..];

    // cell_offset: u32
    let cell_offset = u32::from_le_bytes(reader[..4].try_into().ok()?);
    reader = &reader[4..];

    let r = reader[0];
    let g = reader[1];
    let b = reader[2];
    let checked = reader[3] != 0;

    Some(CheckboxDelta {
        seq,
        chunk_id,
        cell_offset,
        r,
        g,
        b,
        checked,
    })
}

struct CheckboxDelta {
    seq: u64,
    chunk_id: i64,
    cell_offset: u32,
    r: u8,
    g: u8,
    b: u8,
    checked: bool,
}

/// Handle WebSocket close
fn handle_ws_close() {
    with_client(|client| {
        client.reconnect();
    });
}

/// Send message to main thread.
///
/// For chunk data (ChunkInserted/ChunkUpdated), packs into a binary buffer
/// and transfers the ArrayBuffer zero-copy. Format:
///   [tag: u8] [chunk_id: i64 LE] [version: u64 LE] [state: rest of bytes]
/// Tag: 1 = ChunkInserted, 2 = ChunkUpdated
///
/// For small messages (Connected, FatalError), uses JSON.
fn send_to_main_thread(msg: WorkerToMain) {
    let scope = js_sys::global()
        .dyn_into::<DedicatedWorkerGlobalScope>()
        .expect("not in worker");

    match msg {
        WorkerToMain::ChunkInserted { chunk_id, state, version } => {
            send_chunk_binary(&scope, 1, chunk_id, version, &state);
        }
        WorkerToMain::ChunkUpdated { chunk_id, state, version } => {
            send_chunk_binary(&scope, 2, chunk_id, version, &state);
        }
        other => {
            let json = serde_json::to_string(&other).expect("serialization failed");
            let value = wasm_bindgen::JsValue::from_str(&json);
            scope.post_message(&value).expect("postMessage failed");
        }
    }
}

/// Pack chunk data into a binary buffer and transfer to main thread.
fn send_chunk_binary(scope: &DedicatedWorkerGlobalScope, tag: u8, chunk_id: i64, version: u64, state: &[u8]) {
    let t0 = js_sys::Date::now();

    // Header: 1 byte tag + 8 bytes chunk_id + 8 bytes version = 17 bytes
    let total_len = 17 + state.len();
    let buffer = js_sys::ArrayBuffer::new(total_len as u32);
    let view = js_sys::Uint8Array::new(&buffer);

    // Pack header
    let mut header = [0u8; 17];
    header[0] = tag;
    header[1..9].copy_from_slice(&chunk_id.to_le_bytes());
    header[9..17].copy_from_slice(&version.to_le_bytes());
    view.set(&js_sys::Uint8Array::from(&header[..]), 0);

    // Pack state data
    // SAFETY: state slice is valid for the duration of this call
    let state_view = unsafe { js_sys::Uint8Array::view(state) };
    view.set(&state_view, 17);

    let t1 = js_sys::Date::now();

    // Transfer the ArrayBuffer (zero-copy move to main thread)
    let transfer = js_sys::Array::new();
    transfer.push(&buffer);
    scope.post_message_with_transfer(&buffer, &transfer).expect("postMessage failed");

    let t2 = js_sys::Date::now();

    let data_kb = state.len() / 1024;
    if data_kb > 100 {
        web_sys::console::log_1(&format!(
            "[PERF worker->main] pack_buffer={:.0}ms transfer={:.0}ms | {}KB binary",
            t1 - t0, t2 - t1, data_kb
        ).into());
    }
}

/// Encode BSATN arguments for reducer
pub fn encode_update_checkbox_args(
    chunk_id: i64,
    cell_offset: u32,
    r: u8,
    g: u8,
    b: u8,
    checked: bool,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(16);
    buf.extend_from_slice(&chunk_id.to_le_bytes());
    buf.extend_from_slice(&cell_offset.to_le_bytes());
    buf.push(r);
    buf.push(g);
    buf.push(b);
    buf.push(if checked { 1 } else { 0 });
    buf
}

/// Encode batch update arguments
pub fn encode_batch_update_args(updates: &[(i64, u32, u8, u8, u8, bool)]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + updates.len() * 16);
    buf.extend_from_slice(&(updates.len() as u32).to_le_bytes());

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

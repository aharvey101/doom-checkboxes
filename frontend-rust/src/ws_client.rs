//! WebSocket client for SpacetimeDB v2 protocol
//!
//! This module provides a WASM-compatible WebSocket client that speaks
//! the SpacetimeDB binary protocol (v2.bsatn.spacetimedb).

use bytes::Bytes;
use spacetimedb_client_api_messages::websocket::{
    common::QuerySetId,
    v2::{
        CallReducer, CallReducerFlags, ClientMessage, InitialConnection, QueryRows, ReducerOutcome,
        ReducerResult, ServerMessage, Subscribe, SubscribeApplied, SubscriptionError, TableUpdate,
        TableUpdateRows, TransactionUpdate,
    },
};
use spacetimedb_lib::Identity;
use spacetimedb_sats::bsatn;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{BinaryType, CloseEvent, ErrorEvent, MessageEvent, WebSocket};

/// Compression tags for server messages
const COMPRESSION_NONE: u8 = 0;
const COMPRESSION_BROTLI: u8 = 1;
const COMPRESSION_GZIP: u8 = 2;

/// WebSocket protocol identifier
const WS_PROTOCOL: &str = "v2.bsatn.spacetimedb";

/// Connection state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
    Error,
}

/// Callback types for events
pub type OnConnectCallback = Box<dyn FnMut(Identity, String)>;
pub type OnDisconnectCallback = Box<dyn FnMut()>;
pub type OnErrorCallback = Box<dyn FnMut(String)>;
pub type OnRowInsertCallback = Box<dyn FnMut(&[u8])>;
pub type OnRowUpdateCallback = Box<dyn FnMut(&[u8], &[u8])>;
pub type OnRowDeleteCallback = Box<dyn FnMut(&[u8])>;
pub type OnSubscribeAppliedCallback = Box<dyn FnMut()>;
pub type OnReducerResultCallback = Box<dyn FnMut(u32, bool, Option<String>)>;

/// SpacetimeDB WebSocket client
pub struct SpacetimeClient {
    ws: Option<WebSocket>,
    state: ConnectionState,
    request_id: u32,
    identity: Option<Identity>,
    token: Option<String>,

    // Callbacks
    on_connect: Option<OnConnectCallback>,
    on_disconnect: Option<OnDisconnectCallback>,
    on_error: Option<OnErrorCallback>,
    on_subscribe_applied: Option<OnSubscribeAppliedCallback>,

    // Table callbacks (keyed by table name - simplified for checkbox_chunk)
    on_chunk_insert: Option<OnRowInsertCallback>,
    on_chunk_update: Option<OnRowUpdateCallback>,
    on_chunk_delete: Option<OnRowDeleteCallback>,

    // Reducer result callbacks
    on_reducer_result: Option<OnReducerResultCallback>,
}

impl SpacetimeClient {
    pub fn new() -> Self {
        Self {
            ws: None,
            state: ConnectionState::Disconnected,
            request_id: 0,
            identity: None,
            token: None,
            on_connect: None,
            on_disconnect: None,
            on_error: None,
            on_subscribe_applied: None,
            on_chunk_insert: None,
            on_chunk_update: None,
            on_chunk_delete: None,
            on_reducer_result: None,
        }
    }

    /// Get the next request ID
    fn next_request_id(&mut self) -> u32 {
        self.request_id += 1;
        self.request_id
    }

    /// Get connection state
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Get identity if connected
    pub fn identity(&self) -> Option<&Identity> {
        self.identity.as_ref()
    }

    /// Set callback for connection established
    pub fn on_connect(&mut self, callback: OnConnectCallback) {
        self.on_connect = Some(callback);
    }

    /// Set callback for disconnection
    pub fn on_disconnect(&mut self, callback: OnDisconnectCallback) {
        self.on_disconnect = Some(callback);
    }

    /// Set callback for errors
    pub fn on_error(&mut self, callback: OnErrorCallback) {
        self.on_error = Some(callback);
    }

    /// Set callback for subscription applied
    pub fn on_subscribe_applied(&mut self, callback: OnSubscribeAppliedCallback) {
        self.on_subscribe_applied = Some(callback);
    }

    /// Set callback for chunk inserts (checkbox_chunk table)
    pub fn on_chunk_insert(&mut self, callback: OnRowInsertCallback) {
        self.on_chunk_insert = Some(callback);
    }

    /// Set callback for chunk updates (checkbox_chunk table)
    pub fn on_chunk_update(&mut self, callback: OnRowUpdateCallback) {
        self.on_chunk_update = Some(callback);
    }

    /// Set callback for chunk deletes (checkbox_chunk table)
    pub fn on_chunk_delete(&mut self, callback: OnRowDeleteCallback) {
        self.on_chunk_delete = Some(callback);
    }

    /// Set callback for reducer results
    pub fn on_reducer_result(&mut self, callback: OnReducerResultCallback) {
        self.on_reducer_result = Some(callback);
    }
}

/// Shared reference to the client for use in callbacks
pub type SharedClient = Rc<RefCell<SpacetimeClient>>;

/// Connect to SpacetimeDB
pub fn connect(client: SharedClient, uri: &str, database: &str) {
    let url = format!("{}/v1/database/{}/subscribe", uri, database);
    web_sys::console::log_1(&format!("Connecting to: {}", url).into());

    // Create WebSocket with protocol
    let ws = match WebSocket::new_with_str(&url, WS_PROTOCOL) {
        Ok(ws) => ws,
        Err(e) => {
            let error_msg = format!("Failed to create WebSocket: {:?}", e);
            web_sys::console::error_1(&error_msg.clone().into());
            if let Some(cb) = client.borrow_mut().on_error.as_mut() {
                cb(error_msg);
            }
            return;
        }
    };

    ws.set_binary_type(BinaryType::Arraybuffer);

    // Set up event handlers
    let client_clone = client.clone();
    let onmessage_callback = Closure::wrap(Box::new(move |e: MessageEvent| {
        handle_message(&client_clone, e);
    }) as Box<dyn FnMut(MessageEvent)>);
    ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget();

    let client_clone = client.clone();
    let onopen_callback = Closure::wrap(Box::new(move |_| {
        web_sys::console::log_1(&"WebSocket connected".into());
        client_clone.borrow_mut().state = ConnectionState::Connected;
    }) as Box<dyn FnMut(JsValue)>);
    ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
    onopen_callback.forget();

    let client_clone = client.clone();
    let onerror_callback = Closure::wrap(Box::new(move |e: ErrorEvent| {
        let error_msg = format!("WebSocket error: {:?}", e.message());
        web_sys::console::error_1(&error_msg.clone().into());
        client_clone.borrow_mut().state = ConnectionState::Error;
        if let Some(cb) = client_clone.borrow_mut().on_error.as_mut() {
            cb(error_msg);
        }
    }) as Box<dyn FnMut(ErrorEvent)>);
    ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
    onerror_callback.forget();

    let client_clone = client.clone();
    let onclose_callback = Closure::wrap(Box::new(move |_: CloseEvent| {
        web_sys::console::log_1(&"WebSocket closed".into());
        client_clone.borrow_mut().state = ConnectionState::Disconnected;
        client_clone.borrow_mut().ws = None;
        if let Some(cb) = client_clone.borrow_mut().on_disconnect.as_mut() {
            cb();
        }
    }) as Box<dyn FnMut(CloseEvent)>);
    ws.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
    onclose_callback.forget();

    client.borrow_mut().ws = Some(ws);
    client.borrow_mut().state = ConnectionState::Connecting;
}

/// Handle incoming WebSocket message
fn handle_message(client: &SharedClient, event: MessageEvent) {
    let data = event.data();

    // Get ArrayBuffer from the event
    let array_buffer = match data.dyn_into::<js_sys::ArrayBuffer>() {
        Ok(ab) => ab,
        Err(_) => {
            web_sys::console::error_1(&"Expected ArrayBuffer message".into());
            return;
        }
    };

    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
    let mut bytes: Vec<u8> = vec![0; uint8_array.length() as usize];
    uint8_array.copy_to(&mut bytes);

    if bytes.is_empty() {
        return;
    }

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

    // Deserialize the server message
    let message: ServerMessage = match bsatn::from_slice(&decompressed) {
        Ok(msg) => msg,
        Err(e) => {
            web_sys::console::error_1(&format!("Failed to deserialize message: {:?}", e).into());
            return;
        }
    };

    // Handle the message
    handle_server_message(client, message);
}

/// Handle a deserialized server message
fn handle_server_message(client: &SharedClient, message: ServerMessage) {
    match message {
        ServerMessage::InitialConnection(init) => {
            handle_initial_connection(client, init);
        }
        ServerMessage::SubscribeApplied(sub) => {
            handle_subscribe_applied(client, sub);
        }
        ServerMessage::TransactionUpdate(tx) => {
            handle_transaction_update(client, tx);
        }
        ServerMessage::SubscriptionError(err) => {
            handle_subscription_error(client, err);
        }
        ServerMessage::ReducerResult(result) => {
            handle_reducer_result(client, result);
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
fn handle_initial_connection(client: &SharedClient, init: InitialConnection) {
    web_sys::console::log_1(&format!("Connected with identity: {:?}", init.identity).into());

    // Store connection info
    {
        let mut client_mut = client.borrow_mut();
        client_mut.identity = Some(init.identity);
        client_mut.token = Some(init.token.to_string());
        client_mut.state = ConnectionState::Connected;
    }

    // Call callback outside of borrow to avoid RefCell conflicts
    let callback = client.borrow_mut().on_connect.take();
    if let Some(mut cb) = callback {
        cb(init.identity, init.token.to_string());
        // Put callback back
        client.borrow_mut().on_connect = Some(cb);
    }
}

/// Handle subscribe applied message
fn handle_subscribe_applied(client: &SharedClient, sub: SubscribeApplied) {
    web_sys::console::log_1(
        &format!("Subscribe applied for query set {:?}", sub.query_set_id).into(),
    );

    // Process initial rows
    process_query_rows(client, &sub.rows, true);

    // Call callback
    if let Some(cb) = client.borrow_mut().on_subscribe_applied.as_mut() {
        cb();
    }
}

/// Handle transaction update message
fn handle_transaction_update(client: &SharedClient, tx: TransactionUpdate) {
    for query_set in tx.query_sets.iter() {
        for table in query_set.tables.iter() {
            process_table_update(client, table);
        }
    }
}

/// Process a table update
fn process_table_update(client: &SharedClient, table: &TableUpdate) {
    let table_name: &str = &table.table_name;

    // We only care about checkbox_chunk table
    if table_name != "checkbox_chunk" {
        return;
    }

    for rows in table.rows.iter() {
        match rows {
            TableUpdateRows::PersistentTable(persistent) => {
                // Handle inserts
                for row_bytes in &persistent.inserts {
                    if let Some(cb) = client.borrow_mut().on_chunk_insert.as_mut() {
                        cb(&row_bytes);
                    }
                }

                // Handle deletes
                for row_bytes in &persistent.deletes {
                    if let Some(cb) = client.borrow_mut().on_chunk_delete.as_mut() {
                        cb(&row_bytes);
                    }
                }
            }
            TableUpdateRows::EventTable(_) => {
                // Not applicable for checkbox_chunk
            }
        }
    }
}

/// Process query rows (initial subscription data)
fn process_query_rows(client: &SharedClient, rows: &QueryRows, _is_initial: bool) {
    for table_rows in rows.tables.iter() {
        let table_name: &str = &table_rows.table;

        if table_name != "checkbox_chunk" {
            continue;
        }

        for row_bytes in &table_rows.rows {
            if let Some(cb) = client.borrow_mut().on_chunk_insert.as_mut() {
                cb(&row_bytes);
            }
        }
    }
}

/// Handle subscription error
fn handle_subscription_error(client: &SharedClient, err: SubscriptionError) {
    let error_msg = format!("Subscription error: {}", err.error);
    web_sys::console::error_1(&error_msg.clone().into());

    if let Some(cb) = client.borrow_mut().on_error.as_mut() {
        cb(error_msg);
    }
}

/// Handle reducer result
fn handle_reducer_result(client: &SharedClient, result: ReducerResult) {
    let success = matches!(
        &result.result,
        ReducerOutcome::Ok(_) | ReducerOutcome::OkEmpty
    );
    let error = match &result.result {
        ReducerOutcome::Err(bytes) => Some(format!("Reducer error: {:?}", bytes)),
        ReducerOutcome::InternalError(msg) => Some(msg.to_string()),
        _ => None,
    };

    if let Some(cb) = client.borrow_mut().on_reducer_result.as_mut() {
        cb(result.request_id, success, error);
    }

    // Process transaction update if successful
    if let ReducerOutcome::Ok(ok) = result.result {
        for query_set in ok.transaction_update.query_sets.iter() {
            for table in query_set.tables.iter() {
                process_table_update(client, table);
            }
        }
    }
}

/// Subscribe to queries
pub fn subscribe(client: &SharedClient, queries: &[&str]) {
    let mut client_mut = client.borrow_mut();
    let request_id = client_mut.next_request_id();

    let subscribe = Subscribe {
        request_id,
        query_set_id: QuerySetId::new(request_id), // Unique query set ID per subscription
        query_strings: queries.iter().map(|s| (*s).into()).collect(),
    };

    let message = ClientMessage::Subscribe(subscribe);
    send_message(&mut client_mut, &message);
}

/// Call a reducer
pub fn call_reducer(client: &SharedClient, reducer_name: &str, args: &[u8]) -> u32 {
    let mut client_mut = client.borrow_mut();
    let request_id = client_mut.next_request_id();

    let call = CallReducer {
        request_id,
        flags: CallReducerFlags::Default,
        reducer: reducer_name.into(),
        args: Bytes::copy_from_slice(args),
    };

    let message = ClientMessage::CallReducer(call);
    send_message(&mut client_mut, &message);

    request_id
}

/// Send a client message
fn send_message(client: &mut SpacetimeClient, message: &ClientMessage) {
    let Some(ws) = &client.ws else {
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

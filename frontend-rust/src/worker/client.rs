//! Worker-side SpacetimeDB client for Doom Checkboxes
//!
//! Subscribes to snapshot (initial state) and frame (live deltas).

use super::protocol::WorkerToMain;
use bytes::Bytes;
use spacetimedb_client_api_messages::websocket::{
    common::QuerySetId,
    v2::{
        CallReducer, CallReducerFlags, ClientMessage, InitialConnection, QueryRows,
        ReducerOutcome, ReducerResult, ServerMessage, Subscribe, SubscribeApplied,
        SubscriptionError, TableUpdate, TableUpdateRows, TransactionUpdate,
        Unsubscribe, UnsubscribeFlags,
    },
};
use spacetimedb_lib::bsatn;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{DedicatedWorkerGlobalScope, WebSocket};

const BACKOFF_SCHEDULE: [u32; 5] = [5000, 10000, 20000, 40000, 60000];
const MAX_RETRIES: u32 = 5;
const COMPRESSION_NONE: u8 = 0;
const COMPRESSION_BROTLI: u8 = 1;
const COMPRESSION_GZIP: u8 = 2;
const WS_PROTOCOL: &str = "v2.bsatn.spacetimedb";

pub struct WorkerClient {
    ws: Option<WebSocket>,
    uri: String,
    database: String,
    reconnect_attempt: u32,
    intentional_disconnect: bool,
    request_id: u32,
    snapshot_subscription_id: Option<QuerySetId>,
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
            ws: None, uri: String::new(), database: String::new(),
            reconnect_attempt: 0, intentional_disconnect: false, request_id: 0,
            snapshot_subscription_id: None,
            onopen_cb: None, onmessage_cb: None, onerror_cb: None, onclose_cb: None,
        }
    }

    fn next_request_id(&mut self) -> u32 { self.request_id += 1; self.request_id }

    pub fn connect(&mut self, uri: String, database: String) {
        web_sys::console::log_1(&format!("Connecting to {} / {}", uri, database).into());
        self.uri = uri.clone();
        self.database = database.clone();
        self.intentional_disconnect = false;

        if let Some(old) = self.ws.take() { old.close().ok(); }
        self.onopen_cb = None; self.onmessage_cb = None;
        self.onerror_cb = None; self.onclose_cb = None;

        let url = format!("{}/v1/database/{}/subscribe", uri, database);
        let ws = WebSocket::new_with_str(&url, WS_PROTOCOL).expect("WebSocket create failed");
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let onopen = Closure::wrap(Box::new(move |_: web_sys::Event| {
            web_sys::console::log_1(&"WebSocket connected".into());
            CLIENT.with(|c| { if let Some(cl) = c.borrow().as_ref() { cl.borrow_mut().reconnect_attempt = 0; } });
            send_to_main_thread(WorkerToMain::Connected);
        }) as Box<dyn FnMut(_)>);
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        self.onopen_cb = Some(onopen);

        let onmessage = Closure::wrap(Box::new(|e: web_sys::MessageEvent| handle_ws_message(e)) as Box<dyn FnMut(_)>);
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        self.onmessage_cb = Some(onmessage);

        let onerror = Closure::wrap(Box::new(|_: web_sys::Event| { web_sys::console::error_1(&"WebSocket error".into()); }) as Box<dyn FnMut(_)>);
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        self.onerror_cb = Some(onerror);

        let onclose = Closure::wrap(Box::new(|_: web_sys::CloseEvent| {
            web_sys::console::log_1(&"WebSocket closed, reconnecting...".into());
            with_client(|c| c.reconnect());
        }) as Box<dyn FnMut(_)>);
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        self.onclose_cb = Some(onclose);

        self.ws = Some(ws);
    }

    pub fn subscribe(&mut self) {
        let rid = self.next_request_id();
        let snap_qid = QuerySetId::new(rid);
        self.snapshot_subscription_id = Some(snap_qid);
        self.send_message(&ClientMessage::Subscribe(Subscribe {
            request_id: rid, query_set_id: snap_qid,
            query_strings: vec!["SELECT * FROM snapshot".into()].into_boxed_slice(),
        }));

        let rid2 = self.next_request_id();
        self.send_message(&ClientMessage::Subscribe(Subscribe {
            request_id: rid2, query_set_id: QuerySetId::new(rid2),
            query_strings: vec!["SELECT * FROM frame".into()].into_boxed_slice(),
        }));
    }

    fn unsubscribe_snapshot(&mut self) {
        if let Some(qid) = self.snapshot_subscription_id.take() {
            let rid = self.next_request_id();
            self.send_message(&ClientMessage::Unsubscribe(Unsubscribe {
                request_id: rid, query_set_id: qid, flags: UnsubscribeFlags::default(),
            }));
        }
    }

    pub fn call_reducer(&mut self, name: &str, args: &[u8]) -> u32 {
        let rid = self.next_request_id();
        self.send_message(&ClientMessage::CallReducer(CallReducer {
            request_id: rid, flags: CallReducerFlags::Default,
            reducer: name.into(), args: Bytes::copy_from_slice(args),
        }));
        rid
    }

    fn send_message(&self, msg: &ClientMessage) {
        let Some(ws) = &self.ws else { return; };
        if let Ok(bytes) = bsatn::to_vec(msg) {
            let _ = ws.send_with_u8_array(&bytes);
        }
    }

    pub fn disconnect(&mut self) {
        self.intentional_disconnect = true;
        if let Some(ws) = self.ws.take() { ws.close().ok(); }
    }

    pub fn reconnect(&mut self) {
        if self.intentional_disconnect { return; }
        if self.reconnect_attempt >= MAX_RETRIES {
            send_to_main_thread(WorkerToMain::FatalError {
                message: format!("Connection lost after {} retries", MAX_RETRIES),
            });
            return;
        }
        let backoff = BACKOFF_SCHEDULE.get(self.reconnect_attempt as usize).copied().unwrap_or(60000);
        self.reconnect_attempt += 1;
        let uri = self.uri.clone();
        let db = self.database.clone();
        let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
        let closure = Closure::once(Box::new(move || { with_client(|c| c.connect(uri, db)); }) as Box<dyn FnOnce()>);
        let _ = scope.set_timeout_with_callback_and_timeout_and_arguments_0(closure.as_ref().unchecked_ref(), backoff as i32);
        closure.forget();
    }
}

pub fn init_client() {
    CLIENT.with(|c| { *c.borrow_mut() = Some(Rc::new(RefCell::new(WorkerClient::new()))); });
}

pub fn with_client<F: FnOnce(&mut WorkerClient)>(f: F) {
    CLIENT.with(|c| { if let Some(cl) = c.borrow().as_ref() { f(&mut cl.borrow_mut()); } });
}

fn send_to_main_thread(msg: WorkerToMain) {
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let json = serde_json::to_string(&msg).expect("serialize");
    scope.post_message(&JsValue::from_str(&json)).expect("postMessage");
}

fn send_binary_to_main(tag: u8, data: &[u8]) {
    let scope: DedicatedWorkerGlobalScope = js_sys::global().dyn_into().expect("worker");
    let buf = js_sys::ArrayBuffer::new((1 + data.len()) as u32);
    let view = js_sys::Uint8Array::new(&buf);
    view.set_index(0, tag);
    let dv = unsafe { js_sys::Uint8Array::view(data) };
    view.set(&dv, 1);
    let transfer = js_sys::Array::new();
    transfer.push(&buf);
    scope.post_message_with_transfer(&buf, &transfer).expect("postMessage");
}

// === Message handling ===

fn handle_ws_message(event: web_sys::MessageEvent) {
    if let Ok(ab) = event.data().dyn_into::<js_sys::ArrayBuffer>() {
        parse_message(&js_sys::Uint8Array::new(&ab).to_vec());
    }
}

fn parse_message(bytes: &[u8]) {
    if bytes.is_empty() { return; }
    let decompressed = match bytes[0] {
        COMPRESSION_NONE => bytes[1..].to_vec(),
        COMPRESSION_BROTLI => match decompress_brotli(&bytes[1..]) { Ok(d) => d, Err(_) => return },
        COMPRESSION_GZIP => match decompress_gzip(&bytes[1..]) { Ok(d) => d, Err(_) => return },
        _ => return,
    };

    let msg: ServerMessage = match bsatn::from_slice(&decompressed) { Ok(m) => m, Err(_) => return };

    match msg {
        ServerMessage::InitialConnection(init) => {
            web_sys::console::log_1(&format!("Identity: {:?}", init.identity).into());
            send_to_main_thread(WorkerToMain::Connected);
        }
        ServerMessage::SubscribeApplied(sub) => {
            process_initial_rows(&sub.rows);
            with_client(|c| {
                if c.snapshot_subscription_id == Some(sub.query_set_id) { c.unsubscribe_snapshot(); }
            });
        }
        ServerMessage::TransactionUpdate(tx) => {
            for qs in tx.query_sets.iter() { for t in qs.tables.iter() { process_table_update(t); } }
        }
        ServerMessage::ReducerResult(r) => {
            if let ReducerOutcome::Ok(ok) = r.result {
                for qs in ok.transaction_update.query_sets.iter() { for t in qs.tables.iter() { process_table_update(t); } }
            }
        }
        ServerMessage::SubscriptionError(e) => {
            web_sys::console::error_1(&format!("Sub error: {}", e.error).into());
        }
        _ => {}
    }
}

fn process_initial_rows(rows: &QueryRows) {
    for t in rows.tables.iter() {
        if &*t.table == "snapshot" {
            for row in &t.rows { if let Some(s) = parse_snapshot(&row) { send_binary_to_main(1, &s); } }
        }
    }
}

fn process_table_update(table: &TableUpdate) {
    let name: &str = &table.table_name;

    if name == "frame" {
        // Only forward the latest frame — drop older ones to prevent lag
        let mut latest: Option<Vec<u8>> = None;
        for rows in table.rows.iter() {
            if let TableUpdateRows::PersistentTable(p) = rows {
                for row in &p.inserts {
                    if let Some(data) = parse_frame(&row) {
                        latest = Some(data);
                    }
                }
            }
        }
        if let Some(data) = latest {
            send_binary_to_main(2, &data);
        }
    } else if name == "snapshot" {
        for rows in table.rows.iter() {
            if let TableUpdateRows::PersistentTable(p) = rows {
                for row in &p.inserts {
                    if let Some(state) = parse_snapshot(&row) {
                        send_binary_to_main(1, &state);
                    }
                }
            }
        }
    }
}

fn parse_frame(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 12 { return None; }
    let len = u32::from_le_bytes(bytes[8..12].try_into().ok()?) as usize;
    if bytes.len() < 12 + len { return None; }
    Some(bytes[12..12 + len].to_vec())
}

fn parse_snapshot(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 12 { return None; }
    let len = u32::from_le_bytes(bytes[8..12].try_into().ok()?) as usize;
    if bytes.len() < 12 + len { return None; }
    Some(bytes[12..12 + len].to_vec())
}

pub fn encode_send_frame_args(updates: &[(u32, u8, u8, u8, bool)]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + updates.len() * 8);
    buf.extend_from_slice(&(updates.len() as u32).to_le_bytes());
    for (offset, r, g, b, checked) in updates {
        buf.extend_from_slice(&offset.to_le_bytes());
        buf.push(*r); buf.push(*g); buf.push(*b);
        buf.push(if *checked { 1 } else { 0 });
    }
    buf
}

fn decompress_brotli(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut brotli::Decompressor::new(data, 4096), &mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(data).read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

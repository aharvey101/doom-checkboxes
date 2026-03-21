//! Bridge between main thread and worker
//!
//! Provides interface for spawning worker and sending/receiving messages

use crate::worker::protocol::{MainToWorker, WorkerToMain};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MessageEvent, Worker};

thread_local! {
    static WORKER: RefCell<Option<Worker>> = const { RefCell::new(None) };
    static ON_MESSAGE_CALLBACK: RefCell<Option<Closure<dyn FnMut(MessageEvent)>>> = const { RefCell::new(None) };
}

/// Initialize worker and set up message handlers
pub fn init_worker<F>(on_message: F) -> Result<(), String>
where
    F: Fn(WorkerToMain) + 'static,
{
    web_sys::console::log_1(&"[Main] init_worker called".into());

    // Create worker (as ES6 module)
    let mut options = web_sys::WorkerOptions::new();
    options.set_type(web_sys::WorkerType::Module);

    web_sys::console::log_1(&"[Main] Creating worker from worker-loader.js".into());
    let worker = Worker::new_with_options("worker-loader.js", &options)
        .map_err(|e| {
            let err_msg = format!("Failed to create worker: {:?}", e);
            web_sys::console::error_1(&err_msg.clone().into());
            err_msg
        })?;

    web_sys::console::log_1(&"[Main] Worker created successfully".into());

    // Set up message handler
    let callback = Closure::wrap(Box::new(move |event: MessageEvent| {
        let t0 = js_sys::Date::now();
        let data = event.data();

        // Try binary buffer first (chunk data or delta batch), then fall back to JSON
        if let Ok(buffer) = data.clone().dyn_into::<js_sys::ArrayBuffer>() {
            let view = js_sys::Uint8Array::new(&buffer);
            let len = view.length() as usize;
            if len < 1 {
                return;
            }

            let mut tag_byte = [0u8; 1];
            view.slice(0, 1).copy_to(&mut tag_byte);
            let tag = tag_byte[0];

            if tag == 1 || tag == 2 {
                // Chunk data: [tag: 1|2] [chunk_id: i64] [version: u64] [state...]
                if len < 17 { return; }
                let mut header = [0u8; 17];
                view.slice(0, 17).copy_to(&mut header);

                let chunk_id = i64::from_le_bytes(header[1..9].try_into().unwrap());
                let version = u64::from_le_bytes(header[9..17].try_into().unwrap());

                let state_len = len - 17;
                let mut state = vec![0u8; state_len];
                view.slice(17, len as u32).copy_to(&mut state);

                let t1 = js_sys::Date::now();
                if state_len > 100_000 {
                    web_sys::console::log_1(&format!(
                        "[PERF main<-worker] unpack_buffer={:.0}ms | {}KB binary",
                        t1 - t0, state_len / 1024
                    ).into());
                }

                let msg = match tag {
                    1 => WorkerToMain::ChunkInserted { chunk_id, state, version },
                    _ => WorkerToMain::ChunkUpdated { chunk_id, state, version },
                };
                on_message(msg);
            } else if tag == 3 {
                // Delta batch: [tag: 3] [count: u32] [N × 16 bytes]
                if len < 5 { return; }
                let mut count_bytes = [0u8; 4];
                view.slice(1, 5).copy_to(&mut count_bytes);
                let count = u32::from_le_bytes(count_bytes) as usize;

                let data_len = count * 16;
                if len < 5 + data_len { return; }

                let mut bytes = vec![0u8; data_len];
                view.slice(5, (5 + data_len) as u32).copy_to(&mut bytes);

                // Apply deltas directly to loaded_chunks — no full chunk replacement
                // This is the key optimization: small deltas instead of 4MB blobs
                use crate::state::AppState;
                use leptos::prelude::*;

                // We need AppState — get it from the test state or context
                // Since on_message is a closure, we can't easily access AppState here.
                // Instead, create a new message type that the app.rs handler will process.
                // For now, reuse ChunkInserted isn't right. Let's call on_message with
                // a special message. But WorkerToMain doesn't have a delta variant...
                //
                // Simplest approach: apply deltas here using the TEST_STATE thread-local
                // that's already set up.
                crate::app::apply_deltas(&bytes, count);

                let t1 = js_sys::Date::now();
                if count > 100 {
                    web_sys::console::log_1(&format!(
                        "[PERF main<-worker] apply_deltas={:.0}ms | {} deltas",
                        t1 - t0, count
                    ).into());
                }
            } else {
                web_sys::console::error_1(&format!("Unknown binary tag: {}", tag).into());
            }
            return;
        }

        // JSON string message (Connected, FatalError)
        if let Some(json_str) = data.as_string() {
            match serde_json::from_str(&json_str) {
                Ok(m) => on_message(m),
                Err(e) => {
                    web_sys::console::error_1(
                        &format!("Failed to deserialize worker message: {:?}", e).into(),
                    );
                }
            }
            return;
        }

        web_sys::console::error_1(&"Received unknown message type from worker".into());
    }) as Box<dyn FnMut(_)>);

    worker.set_onmessage(Some(callback.as_ref().unchecked_ref()));

    // Store worker and callback
    WORKER.with(|w| {
        *w.borrow_mut() = Some(worker);
    });

    ON_MESSAGE_CALLBACK.with(|c| {
        *c.borrow_mut() = Some(callback);
    });

    Ok(())
}

/// Send message to worker.
///
/// For BatchUpdate, packs into binary buffer and transfers zero-copy.
/// Binary format: [tag: u8 = 3] [updates: N × 16 bytes]
/// Each update: [chunk_id: i64 LE] [cell_offset: u32 LE] [r: u8] [g: u8] [b: u8] [checked: u8]
///
/// For small messages (Connect, Subscribe, etc.), uses JSON.
pub fn send_to_worker(msg: MainToWorker) {
    WORKER.with(|w| {
        if let Some(worker) = w.borrow().as_ref() {
            match msg {
                MainToWorker::BatchUpdate { updates } => {
                    let total_len = 1 + updates.len() * 16;
                    let buffer = js_sys::ArrayBuffer::new(total_len as u32);
                    let view = js_sys::Uint8Array::new(&buffer);

                    // Pack: tag byte + raw update data
                    let mut bytes = Vec::with_capacity(total_len);
                    bytes.push(3u8); // tag for BatchUpdate
                    for (chunk_id, cell_offset, r, g, b, checked) in &updates {
                        bytes.extend_from_slice(&chunk_id.to_le_bytes());
                        bytes.extend_from_slice(&cell_offset.to_le_bytes());
                        bytes.push(*r);
                        bytes.push(*g);
                        bytes.push(*b);
                        bytes.push(if *checked { 1 } else { 0 });
                    }

                    // SAFETY: bytes is valid for duration of this call
                    let src = unsafe { js_sys::Uint8Array::view(&bytes) };
                    view.set(&src, 0);

                    let transfer = js_sys::Array::new();
                    transfer.push(&buffer);
                    let _ = worker.post_message_with_transfer(&buffer, &transfer);
                }
                other => {
                    let Ok(json) = serde_json::to_string(&other) else { return };
                    let value = JsValue::from_str(&json);
                    let _ = worker.post_message(&value);
                }
            }
        }
    });
}

/// Send pre-packed binary buffer to worker (zero-copy transfer).
/// Buffer must already have the tag byte + payload format the worker expects.
pub fn send_binary_to_worker(bytes: &[u8]) {
    WORKER.with(|w| {
        if let Some(worker) = w.borrow().as_ref() {
            let buffer = js_sys::ArrayBuffer::new(bytes.len() as u32);
            let view = js_sys::Uint8Array::new(&buffer);
            // SAFETY: bytes is valid for the duration of this call
            let src = unsafe { js_sys::Uint8Array::view(bytes) };
            view.set(&src, 0);

            let transfer = js_sys::Array::new();
            transfer.push(&buffer);
            let _ = worker.post_message_with_transfer(&buffer, &transfer);
        }
    });
}

/// Send raw JSON string to worker (for testing/performance optimization)
pub fn send_raw_json(json: &str) {
    WORKER.with(|w| {
        if let Some(worker) = w.borrow().as_ref() {
            let value = JsValue::from_str(json);
            let _ = worker.post_message(&value);
        }
    });
}

/// Terminate worker
pub fn terminate_worker() {
    WORKER.with(|w| {
        if let Some(worker) = w.borrow_mut().take() {
            worker.terminate();
        }
    });

    ON_MESSAGE_CALLBACK.with(|c| {
        *c.borrow_mut() = None;
    });
}

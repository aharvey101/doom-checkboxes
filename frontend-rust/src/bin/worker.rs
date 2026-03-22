//! Web worker binary entry point

use checkbox_frontend::worker::client::{encode_batch_update_args, encode_update_checkbox_args, init_client, with_client};
use checkbox_frontend::worker::protocol::MainToWorker;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use web_sys::DedicatedWorkerGlobalScope;

// Worker-side batch accumulator — flushed on a timer
thread_local! {
    static PENDING_UPDATES: RefCell<Vec<(i64, u32, u8, u8, u8, bool)>> = const { RefCell::new(Vec::new()) };
    static FLUSH_SCHEDULED: RefCell<bool> = const { RefCell::new(false) };
}

/// Max updates per SpacetimeDB reducer call to avoid blocking the worker
const MAX_BATCH_SIZE: usize = 50_000;

/// Flush accumulated updates to SpacetimeDB, splitting into chunks if needed
fn flush_worker_batch() {
    let updates = PENDING_UPDATES.with(|p| {
        let mut pending = p.borrow_mut();
        std::mem::take(&mut *pending)
    });
    FLUSH_SCHEDULED.with(|f| *f.borrow_mut() = false);

    if updates.is_empty() {
        return;
    }

    let total = updates.len();
    HAS_FLUSHED.with(|f| *f.borrow_mut() = true);

    // Send in capped batches to avoid blocking the worker on huge messages
    for chunk in updates.chunks(MAX_BATCH_SIZE) {
        let t0 = js_sys::Date::now();
        let args = encode_batch_update_args(chunk);
        let t1 = js_sys::Date::now();
        with_client(|client| {
            client.call_reducer("batch_update_checkboxes", &args);
        });
        let t2 = js_sys::Date::now();
        web_sys::console::log_1(&format!(
            "[PERF worker-flush] encode={:.0}ms send={:.0}ms | {}/{} updates ({}KB bsatn)",
            t1 - t0, t2 - t1, chunk.len(), total, args.len() / 1024
        ).into());
    }
}

/// Schedule a flush if one isn't already pending
fn schedule_flush() {
    let already_scheduled = FLUSH_SCHEDULED.with(|f| {
        let was = *f.borrow();
        *f.borrow_mut() = true;
        was
    });

    if already_scheduled {
        return;
    }

    // Flush after a short delay to allow batching multiple frames
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let closure = Closure::once(Box::new(|| {
        flush_worker_batch();
    }) as Box<dyn FnOnce()>);

    let _ = scope.set_timeout_with_callback_and_timeout_and_arguments_0(
        closure.as_ref().unchecked_ref(),
        16, // ~60 flushes/sec max, keeps batches small
    );
    closure.forget();
}

thread_local! {
    static HAS_FLUSHED: std::cell::RefCell<bool> = const { std::cell::RefCell::new(false) };
}

/// Periodically call snapshot_chunks to persist deltas to full chunk state.
/// Only runs if this client has actually sent updates (i.e., is a player, not a spectator).
fn schedule_snapshot() {
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let closure = Closure::once(Box::new(|| {
        let should_snapshot = HAS_FLUSHED.with(|f| {
            let flushed = *f.borrow();
            *f.borrow_mut() = false;
            flushed
        });

        if should_snapshot {
            with_client(|client| {
                client.call_reducer("snapshot_chunks", &[]);
            });
            // Clean up deltas in a separate reducer call so the
            // TransactionUpdate for snapshot only touches checkbox_chunk
            // (which spectators unsubscribe from).
            // Use u64::MAX to delete all current deltas.
            let cleanup_args = u64::MAX.to_le_bytes();
            with_client(|client| {
                client.call_reducer("cleanup_old_deltas", &cleanup_args);
            });
            web_sys::console::log_1(&"[worker] called snapshot_chunks + cleanup".into());
        }

        schedule_snapshot();
    }) as Box<dyn FnOnce()>);

    let _ = scope.set_timeout_with_callback_and_timeout_and_arguments_0(
        closure.as_ref().unchecked_ref(),
        5000,
    );
    closure.forget();
}

/// Worker entry point - called when worker starts
#[wasm_bindgen(start)]
pub fn worker_main() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"Worker started".into());

    // Initialize client
    init_client();

    // Set up message handler
    let scope = js_sys::global()
        .dyn_into::<DedicatedWorkerGlobalScope>()
        .expect("not in worker context");

    let handler = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
        handle_main_message(event);
    }) as Box<dyn FnMut(_)>);

    scope.set_onmessage(Some(handler.as_ref().unchecked_ref()));
    handler.forget();

    // Start periodic snapshots
    schedule_snapshot();
}

/// Handle messages from main thread
fn handle_main_message(event: web_sys::MessageEvent) {
    let data = event.data();

    // Try binary buffer first (BatchUpdate), then fall back to JSON
    if let Ok(buffer) = data.clone().dyn_into::<js_sys::ArrayBuffer>() {
        let view = js_sys::Uint8Array::new(&buffer);
        let len = view.length() as usize;
        if len < 1 {
            return;
        }

        let mut tag_byte = [0u8; 1];
        view.slice(0, 1).copy_to(&mut tag_byte);

        if tag_byte[0] == 3 {
            // BatchUpdate: parse 16-byte update tuples directly
            let update_count = (len - 1) / 16;
            let mut bytes = vec![0u8; len - 1];
            view.slice(1, len as u32).copy_to(&mut bytes);

            let mut updates = Vec::with_capacity(update_count);
            for i in 0..update_count {
                let offset = i * 16;
                let chunk_id = i64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
                let cell_offset = u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap());
                let r = bytes[offset + 12];
                let g = bytes[offset + 13];
                let b = bytes[offset + 14];
                let checked = bytes[offset + 15] != 0;
                updates.push((chunk_id, cell_offset, r, g, b, checked));
            }

            PENDING_UPDATES.with(|p| p.borrow_mut().extend(updates));
            schedule_flush();
        } else if tag_byte[0] == 4 {
            // DoomFrame: compact format from main thread
            // [tag: 1] [width: 4] [base_x: 4] [base_y: 4] [chunk_id: 8] = 21 byte header
            // [indices: N×4] [colors: N×4]
            if len < 21 { return; }

            let mut header = [0u8; 21];
            view.slice(0, 21).copy_to(&mut header);

            let width = u32::from_le_bytes(header[1..5].try_into().unwrap());
            let base_x = u32::from_le_bytes(header[5..9].try_into().unwrap());
            let base_y = u32::from_le_bytes(header[9..13].try_into().unwrap());
            let chunk_id = i64::from_le_bytes(header[13..21].try_into().unwrap());

            // Remaining bytes: indices then colors, each N×4 bytes
            let remaining = len - 21;
            let pixel_count = remaining / 8; // 4 bytes index + 4 bytes color per pixel
            let indices_end = 21 + pixel_count * 4;

            let mut payload = vec![0u8; remaining];
            view.slice(21, len as u32).copy_to(&mut payload);

            let chunk_size = 1000u32; // CHUNK_SIZE
            let mut updates = Vec::with_capacity(pixel_count);
            for i in 0..pixel_count {
                let idx_off = i * 4;
                let color_off = pixel_count * 4 + i * 4;

                let pixel_idx = u32::from_le_bytes(payload[idx_off..idx_off + 4].try_into().unwrap());
                let x = pixel_idx % width;
                let y = pixel_idx / width;
                let cell_offset = (base_y + y) * chunk_size + (base_x + x);

                let r = payload[color_off];
                let g = payload[color_off + 1];
                let b = payload[color_off + 2];
                let checked = payload[color_off + 3] != 0;

                updates.push((chunk_id, cell_offset, r, g, b, checked));
            }

            PENDING_UPDATES.with(|p| p.borrow_mut().extend(updates));
            schedule_flush();
        }
        return;
    }

    // JSON string message (Connect, Subscribe, UpdateCheckbox, Disconnect)
    let msg: MainToWorker = match data.as_string() {
        Some(json_str) => match serde_json::from_str(&json_str) {
            Ok(m) => m,
            Err(_) => return,
        },
        None => return,
    };

    match msg {
        MainToWorker::Connect { uri, database } => {
            with_client(|client| {
                client.connect(uri, database);
            });
        }
        MainToWorker::Subscribe { chunk_ids } => {
            web_sys::console::log_1(&format!("Subscribe to {} chunks", chunk_ids.len()).into());
            with_client(|client| {
                client.subscribe();
            });
        }
        MainToWorker::UpdateCheckbox {
            chunk_id,
            cell_offset,
            r,
            g,
            b,
            checked,
        } => {
            let args = encode_update_checkbox_args(chunk_id, cell_offset, r, g, b, checked);
            with_client(|client| {
                client.call_reducer("update_checkbox", &args);
            });
        }
        MainToWorker::BatchUpdate { updates } => {
            // Accumulate and schedule flush (same as binary path)
            PENDING_UPDATES.with(|p| p.borrow_mut().extend(updates));
            schedule_flush();
        }
        MainToWorker::Disconnect => {
            with_client(|client| {
                client.disconnect();
            });
        }
    }
}

/// Main entry point (required by Cargo for binary target)
/// When compiled to WASM, the #[wasm_bindgen(start)] function above is the actual entry point
fn main() {}

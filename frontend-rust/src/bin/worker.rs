//! Web worker binary entry point

use checkbox_frontend::worker::client::{encode_batch_update_args, encode_update_checkbox_args, init_client, with_client};
use checkbox_frontend::worker::protocol::MainToWorker;
use wasm_bindgen::prelude::*;
use web_sys::DedicatedWorkerGlobalScope;

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
}

/// Handle messages from main thread
fn handle_main_message(event: web_sys::MessageEvent) {
    web_sys::console::log_1(&"[Worker] Received message from main thread".into());
    let data = event.data();
    web_sys::console::log_1(&format!("[Worker] Message data: {:?}", data).into());

    // Deserialize message
    let msg: MainToWorker = match data.as_string() {
        Some(json_str) => match serde_json::from_str(&json_str) {
            Ok(m) => m,
            Err(_) => return,
        },
        None => return,
    };

    web_sys::console::log_1(&format!("Worker received: {:?}", msg).into());

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
            let args = encode_batch_update_args(&updates);
            with_client(|client| {
                client.call_reducer("batch_update_checkboxes", &args);
            });
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

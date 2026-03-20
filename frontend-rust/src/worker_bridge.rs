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
        // Deserialize message from worker
        let msg: WorkerToMain = match event.data().as_string() {
            Some(json_str) => match serde_json::from_str(&json_str) {
                Ok(m) => m,
                Err(e) => {
                    web_sys::console::error_1(
                        &format!("Failed to deserialize worker message: {:?}", e).into(),
                    );
                    return;
                }
            },
            None => {
                web_sys::console::error_1(&"Received non-string message from worker".into());
                return;
            }
        };

        on_message(msg);
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

/// Send message to worker
pub fn send_to_worker(msg: MainToWorker) {
    WORKER.with(|w| {
        if let Some(worker) = w.borrow().as_ref() {
            let Ok(json) = serde_json::to_string(&msg) else { return };
            let value = JsValue::from_str(&json);
            let _ = worker.post_message(&value);
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

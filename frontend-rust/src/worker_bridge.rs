//! Bridge between main thread and worker for Doom Checkboxes

use crate::worker::protocol::{MainToWorker, WorkerToMain};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MessageEvent, Worker};

thread_local! {
    static WORKER: RefCell<Option<Worker>> = const { RefCell::new(None) };
    static ON_MESSAGE_CALLBACK: RefCell<Option<Closure<dyn FnMut(MessageEvent)>>> = const { RefCell::new(None) };
}

pub fn init_worker<F>(on_message: F) -> Result<(), String>
where
    F: Fn(WorkerToMain) + 'static,
{
    let mut options = web_sys::WorkerOptions::new();
    options.set_type(web_sys::WorkerType::Module);

    let worker = Worker::new_with_options("worker-loader.js", &options)
        .map_err(|e| format!("Failed to create worker: {:?}", e))?;

    let callback = Closure::wrap(Box::new(move |event: MessageEvent| {
        let data = event.data();

        // Binary messages from worker
        if let Ok(buffer) = data.clone().dyn_into::<js_sys::ArrayBuffer>() {
            let view = js_sys::Uint8Array::new(&buffer);
            let len = view.length() as usize;
            if len < 1 { return; }

            let mut tag = [0u8; 1];
            view.slice(0, 1).copy_to(&mut tag);

            match tag[0] {
                1 => {
                    // Snapshot: [tag=1] [state: 640*400*4 bytes]
                    let state_len = len - 1;
                    let mut state = vec![0u8; state_len];
                    view.slice(1, len as u32).copy_to(&mut state);
                    crate::app::apply_snapshot(&state);
                }
                2 => {
                    // Frame delta: [tag=2] [N × 7 bytes: offset3 + rgba]
                    let delta_len = len - 1;
                    let mut data = vec![0u8; delta_len];
                    view.slice(1, len as u32).copy_to(&mut data);
                    crate::app::apply_frame_delta(&data);
                }
                _ => {}
            }
            return;
        }

        // JSON messages (Connected, FatalError)
        if let Some(json_str) = data.as_string() {
            if let Ok(m) = serde_json::from_str(&json_str) {
                on_message(m);
            }
        }
    }) as Box<dyn FnMut(_)>);

    worker.set_onmessage(Some(callback.as_ref().unchecked_ref()));

    WORKER.with(|w| { *w.borrow_mut() = Some(worker); });
    ON_MESSAGE_CALLBACK.with(|c| { *c.borrow_mut() = Some(callback); });

    Ok(())
}

pub fn send_to_worker(msg: MainToWorker) {
    WORKER.with(|w| {
        if let Some(worker) = w.borrow().as_ref() {
            let Ok(json) = serde_json::to_string(&msg) else { return };
            let _ = worker.post_message(&JsValue::from_str(&json));
        }
    });
}

/// Send pre-packed binary buffer to worker (zero-copy transfer)
pub fn send_binary_to_worker(bytes: &[u8]) {
    WORKER.with(|w| {
        if let Some(worker) = w.borrow().as_ref() {
            let buffer = js_sys::ArrayBuffer::new(bytes.len() as u32);
            let view = js_sys::Uint8Array::new(&buffer);
            let src = unsafe { js_sys::Uint8Array::view(bytes) };
            view.set(&src, 0);
            let transfer = js_sys::Array::new();
            transfer.push(&buffer);
            let _ = worker.post_message_with_transfer(&buffer, &transfer);
        }
    });
}

pub fn terminate_worker() {
    WORKER.with(|w| { if let Some(worker) = w.borrow_mut().take() { worker.terminate(); } });
    ON_MESSAGE_CALLBACK.with(|c| { *c.borrow_mut() = None; });
}

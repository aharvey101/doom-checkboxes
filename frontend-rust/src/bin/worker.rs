//! Web worker binary entry point for Doom Checkboxes

use checkbox_frontend::worker::client::{encode_send_frame_args, init_client, with_client};
use checkbox_frontend::worker::protocol::MainToWorker;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use web_sys::DedicatedWorkerGlobalScope;

// Batch accumulator for doom frame pixels
thread_local! {
    static PENDING_UPDATES: RefCell<Vec<(u32, u8, u8, u8, bool)>> = const { RefCell::new(Vec::new()) };
    static FLUSH_SCHEDULED: RefCell<bool> = const { RefCell::new(false) };
}

fn flush_batch() {
    let updates = PENDING_UPDATES.with(|p| std::mem::take(&mut *p.borrow_mut()));
    FLUSH_SCHEDULED.with(|f| *f.borrow_mut() = false);

    if updates.is_empty() { return; }

    let args = encode_send_frame_args(&updates);
    with_client(|c| { c.call_reducer("send_frame", &args); });
}

fn schedule_flush() {
    let already = FLUSH_SCHEDULED.with(|f| { let was = *f.borrow(); *f.borrow_mut() = true; was });
    if already { return; }

    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let closure = Closure::once(Box::new(|| flush_batch()) as Box<dyn FnOnce()>);
    let _ = scope.set_timeout_with_callback_and_timeout_and_arguments_0(
        closure.as_ref().unchecked_ref(), 16);
    closure.forget();
}

/// Periodic cleanup of old frame rows
fn schedule_cleanup() {
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let closure = Closure::once(Box::new(|| {
        let args = u64::MAX.to_le_bytes();
        with_client(|c| { c.call_reducer("cleanup_frames", &args); });
        schedule_cleanup();
    }) as Box<dyn FnOnce()>);
    let _ = scope.set_timeout_with_callback_and_timeout_and_arguments_0(
        closure.as_ref().unchecked_ref(), 10000);
    closure.forget();
}

#[wasm_bindgen(start)]
pub fn worker_main() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"Worker started".into());

    init_client();
    schedule_cleanup();

    let scope = js_sys::global().dyn_into::<DedicatedWorkerGlobalScope>().expect("not worker");
    let handler = Closure::wrap(Box::new(|event: web_sys::MessageEvent| {
        handle_main_message(event);
    }) as Box<dyn FnMut(_)>);
    scope.set_onmessage(Some(handler.as_ref().unchecked_ref()));
    handler.forget();
}

fn handle_main_message(event: web_sys::MessageEvent) {
    let data = event.data();

    // Binary: tag=4 DoomFrame from main thread
    if let Ok(buffer) = data.clone().dyn_into::<js_sys::ArrayBuffer>() {
        let view = js_sys::Uint8Array::new(&buffer);
        let len = view.length() as usize;
        if len < 1 { return; }

        let mut tag = [0u8; 1];
        view.slice(0, 1).copy_to(&mut tag);

        if tag[0] == 4 {
            // DoomFrame: [tag=4][width:4][base_x:4][base_y:4][chunk_id:8] = 21 header
            // then [indices: N×4][colors: N×4]
            if len < 21 { return; }
            let mut header = [0u8; 21];
            view.slice(0, 21).copy_to(&mut header);

            let width = u32::from_le_bytes(header[1..5].try_into().unwrap());
            let base_x = u32::from_le_bytes(header[5..9].try_into().unwrap());
            let base_y = u32::from_le_bytes(header[9..13].try_into().unwrap());

            let remaining = len - 21;
            let pixel_count = remaining / 8;

            let mut payload = vec![0u8; remaining];
            view.slice(21, len as u32).copy_to(&mut payload);

            let chunk_size = 1000u32;
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

                updates.push((cell_offset, r, g, b, checked));
            }

            PENDING_UPDATES.with(|p| p.borrow_mut().extend(updates));
            schedule_flush();
        }
        return;
    }

    // JSON messages
    let msg: MainToWorker = match data.as_string() {
        Some(s) => match serde_json::from_str(&s) { Ok(m) => m, Err(_) => return },
        None => return,
    };

    match msg {
        MainToWorker::Connect { uri, database } => {
            with_client(|c| c.connect(uri, database));
        }
        MainToWorker::Subscribe { .. } => {
            with_client(|c| c.subscribe());
        }
        MainToWorker::Disconnect => {
            with_client(|c| c.disconnect());
        }
        _ => {}
    }
}

fn main() {}

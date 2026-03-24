//! Doom Mode - captures Doom frames and broadcasts via SpacetimeDB

use crate::state::AppState;
use leptos::prelude::*;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

const DOOM_WIDTH: u32 = 640;

thread_local! {
    static FRAME_CALLBACK: RefCell<Option<Closure<dyn FnMut(js_sys::Uint32Array, js_sys::Uint8Array, u32, u32, i32, i32)>>> = const { RefCell::new(None) };
    static DOOM_STATE: RefCell<Option<AppState>> = const { RefCell::new(None) };
    static FPS_FRAME_COUNT: RefCell<u32> = const { RefCell::new(0) };
    static FPS_LAST_LOG: RefCell<f64> = const { RefCell::new(0.0) };
}

// JS bindings for DoomMode
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = DoomMode)]
    fn init(container_id: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = DoomMode)]
    fn startCapture(callback: &Closure<dyn FnMut(js_sys::Uint32Array, js_sys::Uint8Array, u32, u32, i32, i32)>);

    #[wasm_bindgen(js_namespace = DoomMode)]
    fn stopCapture();

    #[wasm_bindgen(js_namespace = DoomMode)]
    fn stop();

    #[wasm_bindgen(js_namespace = DoomMode)]
    fn isRunning() -> bool;

    #[wasm_bindgen(js_namespace = DoomMode)]
    fn toggleControls() -> bool;

    #[wasm_bindgen(js_namespace = DoomMode)]
    fn enableControls();
}

fn is_doom_available() -> bool {
    let window = web_sys::window().expect("no window");
    js_sys::Reflect::has(&window, &JsValue::from_str("DoomMode")).unwrap_or(false)
}

fn set_doom_container_visible(visible: bool) {
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        if let Some(el) = doc.get_element_by_id("doom-container") {
            if let Some(style) = el.dyn_ref::<web_sys::HtmlElement>().map(|e| e.style()) {
                let _ = style.set_property("display", if visible { "block" } else { "none" });
            }
        }
    }
}

/// Start Doom mode
pub async fn start_doom_mode(state: AppState) -> Result<(), String> {
    if !is_doom_available() {
        return Err("DoomMode not available".to_string());
    }

    // Clear pixel buffer
    crate::app::PIXEL_BUFFER.with(|buf| buf.borrow_mut().fill(0));
    state.render_version.update(|v| *v += 1);

    DOOM_STATE.with(|s| { *s.borrow_mut() = Some(state); });
    set_doom_container_visible(true);

    let promise = init("doom-container");
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(_) => {
            let callback = Closure::wrap(Box::new(
                move |indices: js_sys::Uint32Array, colors: js_sys::Uint8Array, width: u32, height: u32, _ox: i32, _oy: i32| {
                    handle_frame(indices, colors, width, height);
                },
            ) as Box<dyn FnMut(js_sys::Uint32Array, js_sys::Uint8Array, u32, u32, i32, i32)>);

            startCapture(&callback);
            FRAME_CALLBACK.with(|c| { *c.borrow_mut() = Some(callback); });
            Ok(())
        }
        Err(e) => {
            set_doom_container_visible(false);
            Err(format!("Failed to init Doom: {:?}", e))
        }
    }
}

/// Stop Doom mode
pub fn stop_doom_mode() {
    if is_doom_available() { stopCapture(); stop(); }
    set_doom_container_visible(false);
    FRAME_CALLBACK.with(|c| { *c.borrow_mut() = None; });
    DOOM_STATE.with(|s| { *s.borrow_mut() = None; });
}

pub fn is_doom_running() -> bool {
    if is_doom_available() { isRunning() } else { false }
}

pub fn toggle_doom_controls() {
    if is_doom_available() { toggleControls(); }
}

/// Handle a doom frame delta
fn handle_frame(
    indices: js_sys::Uint32Array,
    color_data: js_sys::Uint8Array,
    width: u32,
    _height: u32,
) {
    // FPS tracking
    let now = js_sys::Date::now();
    FPS_FRAME_COUNT.with(|c| *c.borrow_mut() += 1);
    FPS_LAST_LOG.with(|last| {
        let prev = *last.borrow();
        if prev == 0.0 { *last.borrow_mut() = now; }
        else if now - prev >= 1000.0 {
            let frames = FPS_FRAME_COUNT.with(|c| { let f = *c.borrow(); *c.borrow_mut() = 0; f });
            web_sys::console::log_1(&format!(
                "[PERF doom] {:.1} FPS ({} frames)", frames as f64 / ((now - prev) / 1000.0), frames
            ).into());
            *last.borrow_mut() = now;
        }
    });

    let state = DOOM_STATE.with(|s| s.borrow().clone());
    let Some(state) = state else { return; };

    let pixel_count = indices.length() as usize;
    let indices_vec: Vec<u32> = indices.to_vec();
    let color_vec: Vec<u8> = color_data.to_vec();
    if indices_vec.is_empty() { return; }

    // Optimistic render to pixel buffer
    crate::app::PIXEL_BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();
        let buf_len = buf.len();

        for (i, &pixel_idx) in indices_vec.iter().enumerate() {
            let byte_idx = (pixel_idx as usize) * 4;
            let color_idx = i * 4;

            if byte_idx + 3 < buf_len && color_idx + 3 < color_vec.len() {
                buf[byte_idx] = color_vec[color_idx];
                buf[byte_idx + 1] = color_vec[color_idx + 1];
                buf[byte_idx + 2] = color_vec[color_idx + 2];
                buf[byte_idx + 3] = if color_vec[color_idx + 3] != 0 { 0xFF } else { 0x00 };
            }
        }
    });
    state.render_version.update(|v| *v += 1);

    // Send to worker for SpacetimeDB broadcast
    // Binary: [tag=4][width:u32][indices: N×u32][colors: N×4 u8]
    let header_len = 1 + 4;
    let total = header_len + pixel_count * 4 + pixel_count * 4;

    let mut buf = Vec::with_capacity(total);
    buf.push(4u8);
    buf.extend_from_slice(&width.to_le_bytes());
    for &idx in &indices_vec {
        buf.extend_from_slice(&idx.to_le_bytes());
    }
    buf.extend_from_slice(&color_vec);

    crate::worker_bridge::send_binary_to_worker(&buf);
}

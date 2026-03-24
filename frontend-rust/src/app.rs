use leptos::prelude::*;
use std::cell::Cell;
use wasm_bindgen::JsCast;

use crate::components::Header;
use crate::state::AppState;
use crate::worker_bridge::{init_worker, send_to_worker, terminate_worker};
use crate::worker_protocol::{MainToWorker, WorkerToMain};

const STYLES: &str = include_str!("styles.css");

/// Doom frame dimensions
const DOOM_WIDTH: usize = 640;
const DOOM_HEIGHT: usize = 400;
const PIXEL_COUNT: usize = DOOM_WIDTH * DOOM_HEIGHT;
const BUFFER_SIZE: usize = PIXEL_COUNT * 4; // RGBA

#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();
    set_test_state(state);

    // Initialize worker once
    Effect::new(move || {
        thread_local! {
            static INIT: Cell<bool> = const { Cell::new(false) };
        }

        INIT.with(|init| {
            if init.get() { return; }
            init.set(true);

            let subscribed = std::cell::Cell::new(false);
            let result = init_worker(move |msg| {
                match msg {
                    WorkerToMain::Connected => {
                        state.status.set(crate::state::ConnectionStatus::Connected);
                        state.status_message.set("Connected".to_string());

                        if !subscribed.get() {
                            subscribed.set(true);
                            send_to_worker(MainToWorker::Subscribe { chunk_ids: vec![] });
                        }
                    }
                    WorkerToMain::FatalError { message } => {
                        state.status.set(crate::state::ConnectionStatus::Error);
                        state.status_message.set(message);
                    }
                    _ => {}
                }
            });

            if let Err(e) = result {
                web_sys::console::error_1(&format!("Worker init failed: {}", e).into());
                return;
            }

            // Connect to SpacetimeDB
            let uri = get_spacetimedb_uri();
            let closure = wasm_bindgen::closure::Closure::once(move || {
                send_to_worker(MainToWorker::Connect {
                    uri,
                    database: "doom-checkboxes".to_string(),
                });
            });
            web_sys::window().unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    closure.as_ref().unchecked_ref(), 100).ok();
            closure.forget();
        });
    });

    on_cleanup(|| { terminate_worker(); });

    view! {
        <style>{STYLES}</style>
        <Header state=state />
        <DoomCanvas state=state />
    }
}

/// Canvas component that renders the Doom pixel buffer
#[component]
fn DoomCanvas(state: AppState) -> impl IntoView {
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();

    // Render when buffer changes
    Effect::new(move |_| {
        let _version = state.render_version.get();
        let Some(canvas) = canvas_ref.get() else { return };
        let canvas: &web_sys::HtmlCanvasElement = &canvas;

        canvas.set_width(DOOM_WIDTH as u32);
        canvas.set_height(DOOM_HEIGHT as u32);

        let ctx = canvas.get_context("2d").ok().flatten()
            .and_then(|c| c.dyn_into::<web_sys::CanvasRenderingContext2d>().ok());
        let Some(ctx) = ctx else { return };

        PIXEL_BUFFER.with(|buf| {
            let buf = buf.borrow();
            let clamped = wasm_bindgen::Clamped(&buf[..]);
            if let Ok(image_data) = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
                clamped, DOOM_WIDTH as u32, DOOM_HEIGHT as u32,
            ) {
                ctx.put_image_data(&image_data, 0.0, 0.0).ok();
            }
        });
    });

    view! {
        <canvas
            node_ref=canvas_ref
            width=DOOM_WIDTH
            height=DOOM_HEIGHT
            style="image-rendering: pixelated; width: 100%; max-width: 960px; display: block; margin: 20px auto; background: #000;"
        />
    }
}

fn get_spacetimedb_uri() -> String {
    let window = web_sys::window().expect("no window");
    let hostname = window.location().hostname().unwrap_or_default();
    if hostname == "localhost" || hostname == "127.0.0.1" {
        "ws://127.0.0.1:3000".to_string()
    } else {
        "wss://maincloud.spacetimedb.com".to_string()
    }
}

// === Pixel buffer ===

thread_local! {
    pub static PIXEL_BUFFER: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(vec![0u8; BUFFER_SIZE]);
    static TEST_STATE: std::cell::RefCell<Option<AppState>> = const { std::cell::RefCell::new(None) };
    static RENDER_SCHEDULED: std::cell::RefCell<bool> = const { std::cell::RefCell::new(false) };
}

fn set_test_state(state: AppState) {
    TEST_STATE.with(|s| { *s.borrow_mut() = Some(state); });
}

fn schedule_render() {
    let already = RENDER_SCHEDULED.with(|f| { let was = *f.borrow(); *f.borrow_mut() = true; was });
    if already { return; }

    TEST_STATE.with(|s| {
        let state = s.borrow();
        let Some(state) = state.as_ref() else { return };
        let state_copy = *state;

        let closure = wasm_bindgen::closure::Closure::once(Box::new(move || {
            RENDER_SCHEDULED.with(|f| *f.borrow_mut() = false);
            state_copy.render_version.update(|v| *v += 1);
        }) as Box<dyn FnOnce()>);

        web_sys::window().expect("window")
            .request_animation_frame(closure.as_ref().unchecked_ref()).ok();
        closure.forget();
    });
}

/// Apply full snapshot (640×400×4 = 1,024,000 bytes RGBA)
pub fn apply_snapshot(data: &[u8]) {
    PIXEL_BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();
        let len = data.len().min(buf.len());
        buf[..len].copy_from_slice(&data[..len]);
    });
    schedule_render();
}

/// Apply frame delta: packed [N × 7 bytes: offset_hi, offset_mid, offset_lo, r, g, b, checked]
pub fn apply_frame_delta(data: &[u8]) {
    let count = data.len() / 7;
    if count == 0 { return; }

    PIXEL_BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();
        for i in 0..count {
            let off = i * 7;
            if off + 6 >= data.len() { break; }

            let pixel_offset = ((data[off] as u32) << 16)
                | ((data[off + 1] as u32) << 8)
                | (data[off + 2] as u32);
            let byte_idx = (pixel_offset as usize) * 4;

            if byte_idx + 3 < buf.len() {
                buf[byte_idx] = data[off + 3];     // R
                buf[byte_idx + 1] = data[off + 4]; // G
                buf[byte_idx + 2] = data[off + 5]; // B
                buf[byte_idx + 3] = if data[off + 6] != 0 { 0xFF } else { 0x00 }; // A
            }
        }
    });
    schedule_render();
}

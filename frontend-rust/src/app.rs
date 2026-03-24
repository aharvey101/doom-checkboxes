use leptos::prelude::*;
use std::cell::Cell;
use wasm_bindgen::JsCast;

use crate::bookmark::{load_viewport, parse_bookmark, save_viewport};
use crate::components::{CheckboxCanvas, Header};
use crate::constants::CELL_SIZE;
use crate::state::AppState;
use crate::worker_bridge::{init_worker, send_to_worker, terminate_worker};
use crate::worker_protocol::{MainToWorker, WorkerToMain};

const STYLES: &str = include_str!("styles.css");

#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();

    // Store state for testing
    set_test_state(state);

    // Parse bookmark URL on mount and set initial viewport
    Effect::new(move || {
        if let Some(window) = web_sys::window() {
            if let Ok(search) = window.location().search() {
                let bookmark = parse_bookmark(&search);

                // If URL has coordinates, use those (for shared links)
                if bookmark.x != 0.0 || bookmark.y != 0.0 || bookmark.zoom != 1.0 {
                    // Get canvas size (approximate, will be corrected on resize)
                    let canvas_w = window
                        .inner_width()
                        .ok()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(1200.0)
                        - 40.0;
                    let canvas_h = window
                        .inner_height()
                        .ok()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(800.0)
                        - 120.0;

                    let scale = bookmark.zoom;
                    let cell_size = CELL_SIZE * scale;

                    // Center the bookmark position on screen
                    let offset_x = canvas_w / 2.0 - (bookmark.x as f64) * cell_size;
                    let offset_y = canvas_h / 2.0 - (bookmark.y as f64) * cell_size;

                    state.offset_x.set(offset_x);
                    state.offset_y.set(offset_y);
                    state.scale.set(scale);

                    web_sys::console::log_1(
                        &format!(
                            "Loaded bookmark: ({}, {}) zoom={}",
                            bookmark.x, bookmark.y, bookmark.zoom
                        )
                        .into(),
                    );
                } else if let Some((offset_x, offset_y, scale)) = load_viewport() {
                    // No URL params - restore from localStorage
                    state.offset_x.set(offset_x);
                    state.offset_y.set(offset_y);
                    state.scale.set(scale);

                    web_sys::console::log_1(
                        &format!(
                            "Restored viewport from localStorage: offset=({}, {}), scale={}",
                            offset_x, offset_y, scale
                        )
                        .into(),
                    );
                }
            }
        }

        // Initialize worker (with once-only guard to prevent re-initialization)
        thread_local! {
            static WORKER_INITIALIZED: Cell<bool> = const { Cell::new(false) };
        }

        WORKER_INITIALIZED.with(|initialized| {
            if !initialized.get() {
                initialized.set(true);

                let result = init_worker(move |msg| {
                    web_sys::console::log_1(&format!("[Main] Received message from worker: {:?}", msg).into());
                    match msg {
                        WorkerToMain::Connected => {
                            web_sys::console::log_1(&"[Main] Worker connected!".into());
                            state.status.set(crate::state::ConnectionStatus::Connected);
                            state.status_message.set("Connected".to_string());

                            // Subscribe to checkbox chunks
                            web_sys::console::log_1(&"[Main] Sending Subscribe message to worker".into());
                            send_to_worker(MainToWorker::Subscribe { chunk_ids: vec![] });
                        }
                        WorkerToMain::ChunkInserted { chunk_id, state: chunk_state, version } => {
                            let t0 = js_sys::Date::now();
                            let data_kb = chunk_state.len() / 1024;
                            // Only ignore doom chunk updates when WE are running Doom locally,
                            // to prevent server round-trips from overwriting optimistic frames.
                            // Other users' Doom frames should still be visible.
                            if !crate::doom::is_doom_chunk(chunk_id) || !crate::doom::is_doom_running() {
                                state.loaded_chunks.update(|chunks| {
                                    chunks.insert(chunk_id, chunk_state);
                                });
                                state.render_version.update(|v| *v += 1);
                            }
                            state.subscribed_chunks.update(|subs| {
                                subs.insert(chunk_id);
                            });
                            state.loading_chunks.update(|loading| {
                                loading.remove(&chunk_id);
                            });
                            let t1 = js_sys::Date::now();
                            if data_kb > 100 {
                                web_sys::console::log_1(&format!(
                                    "[PERF main] chunk {} inserted state_update={:.0}ms | {}KB",
                                    chunk_id, t1 - t0, data_kb
                                ).into());
                            }
                        }
                        WorkerToMain::ChunkUpdated { chunk_id, state: chunk_state, version } => {
                            web_sys::console::log_1(&format!("Chunk {} updated, version {}", chunk_id, version).into());
                            // Only ignore doom chunk updates when WE are running Doom locally.
                            if !crate::doom::is_doom_chunk(chunk_id) || !crate::doom::is_doom_running() {
                                state.loaded_chunks.update(|chunks| {
                                    chunks.insert(chunk_id, chunk_state);
                                });
                                state.render_version.update(|v| *v += 1);
                            }
                        }
                        WorkerToMain::FatalError { message } => {
                            state.status.set(crate::state::ConnectionStatus::Error);
                            state.status_message.set(message);
                        }
                    }
                });

                match result {
                    Ok(_) => web_sys::console::log_1(&"[Main] Worker initialized successfully".into()),
                    Err(e) => {
                        web_sys::console::error_1(&format!("[Main] Worker init failed: {}", e).into());
                        return;
                    }
                }

                // Connect to SpacetimeDB via worker (with delay to ensure worker is ready)
                let uri = get_spacetimedb_uri();
                let callback = wasm_bindgen::closure::Closure::once(move || {
                    web_sys::console::log_1(&format!("[Main] Sending Connect message to worker: {} / doom-checkboxes", uri).into());
                    send_to_worker(MainToWorker::Connect {
                        uri,
                        database: "doom-checkboxes".to_string(),
                    });
                });
                web_sys::window()
                    .unwrap()
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        callback.as_ref().unchecked_ref(),
                        100
                    )
                    .ok();
                callback.forget();
            }
        });
    });

    // Cleanup: terminate worker when component unmounts
    on_cleanup(|| {
        terminate_worker();
    });

    // Save viewport to localStorage when it changes (debounced via effect)
    Effect::new(move |_| {
        let offset_x = state.offset_x.get();
        let offset_y = state.offset_y.get();
        let scale = state.scale.get();

        save_viewport(offset_x, offset_y, scale);
    });

    view! {
        <style>{STYLES}</style>
        <Header state=state />
        <CheckboxCanvas state=state />
    }
}

/// Get the SpacetimeDB URI based on environment
fn get_spacetimedb_uri() -> String {
    let window = web_sys::window().expect("no window");
    let location = window.location();
    let hostname = location.hostname().unwrap_or_default();

    if hostname == "localhost" || hostname == "127.0.0.1" {
        "ws://127.0.0.1:3000".to_string()
    } else {
        "wss://maincloud.spacetimedb.com".to_string()
    }
}

// Expose for testing
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn test_send_batch_update(updates_js: wasm_bindgen::JsValue) -> Result<(), wasm_bindgen::JsValue> {
    use crate::worker_bridge;

    // Serialize updates to JSON string directly without deserializing to Rust first
    // This minimizes main thread blocking (avoids expensive serde_wasm_bindgen::from_value)
    let updates_json = js_sys::JSON::stringify(&updates_js)
        .map_err(|e| wasm_bindgen::JsValue::from_str(&format!("Failed to stringify updates: {:?}", e)))?
        .as_string()
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("Failed to convert to string"))?;

    // Manually construct the message JSON to avoid Rust serialization overhead
    let msg_json = format!(r#"{{"BatchUpdate":{{"updates":{}}}}}"#, updates_json);

    // Send the raw JSON to worker (bypasses normal send_to_worker to avoid double serialization)
    worker_bridge::send_raw_json(&msg_json);

    Ok(())
}

// Global state for testing
thread_local! {
    static TEST_STATE: std::cell::RefCell<Option<AppState>> = const { std::cell::RefCell::new(None) };
}

// Store state reference for testing
pub fn set_test_state(state: AppState) {
    TEST_STATE.with(|s| {
        *s.borrow_mut() = Some(state);
    });
}

thread_local! {
    static DELTA_RENDER_SCHEDULED: std::cell::RefCell<bool> = const { std::cell::RefCell::new(false) };
}

/// Apply delta updates from the worker directly to loaded_chunks.
/// Called from worker_bridge when a DeltaBatch binary message arrives.
/// bytes: packed [N × 16 bytes: chunk_id(8) + cell_offset(4) + r + g + b + checked]
///
/// Coalesces renders: only bumps render_version once per animation frame,
/// even if multiple delta batches arrive in the same frame.
thread_local! {
    static DELTA_BATCH_COUNT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static DELTA_TOTAL_UPDATES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Get total delta batches received (for benchmarking)
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn get_delta_batch_count() -> u32 {
    DELTA_BATCH_COUNT.with(|c| c.get())
}

/// Get total individual delta updates applied (for benchmarking)
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn get_delta_total_updates() -> u32 {
    DELTA_TOTAL_UPDATES.with(|c| c.get())
}

pub fn apply_deltas(bytes: &[u8], count: usize) {
    DELTA_BATCH_COUNT.with(|c| c.set(c.get() + 1));
    DELTA_TOTAL_UPDATES.with(|c| c.set(c.get() + count as u32));
    TEST_STATE.with(|s| {
        let state = s.borrow();
        let Some(state) = state.as_ref() else { return };

        state.loaded_chunks.update(|chunks| {
            use crate::constants::CHUNK_DATA_SIZE;

            // Cache last chunk pointer to avoid HashMap lookup per delta.
            // Nearly all doom deltas hit the same chunk_id.
            let mut last_chunk_id: i64 = i64::MIN;
            let mut last_data: *mut Vec<u8> = std::ptr::null_mut();

            for i in 0..count {
                let offset = i * 16;
                if offset + 16 > bytes.len() { break; }

                let chunk_id = i64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
                let cell_offset = u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap());

                // Only do HashMap lookup when chunk_id changes
                if chunk_id != last_chunk_id {
                    last_chunk_id = chunk_id;
                    let data = chunks.entry(chunk_id).or_insert_with(|| vec![0u8; CHUNK_DATA_SIZE]);
                    last_data = data as *mut Vec<u8>;
                }

                let data = unsafe { &mut *last_data };
                let byte_idx = (cell_offset as usize) * 4;
                if byte_idx + 3 < data.len() {
                    data[byte_idx] = bytes[offset + 12];
                    data[byte_idx + 1] = bytes[offset + 13];
                    data[byte_idx + 2] = bytes[offset + 14];
                    data[byte_idx + 3] = if bytes[offset + 15] != 0 { 0xFF } else { 0x00 };
                }
            }
        });

        // Coalesce: schedule one render_version bump per animation frame
        let already_scheduled = DELTA_RENDER_SCHEDULED.with(|f| {
            let was = *f.borrow();
            *f.borrow_mut() = true;
            was
        });

        if !already_scheduled {
            let state_copy = *state;
            let closure = wasm_bindgen::closure::Closure::once(Box::new(move || {
                DELTA_RENDER_SCHEDULED.with(|f| *f.borrow_mut() = false);
                state_copy.render_version.update(|v| *v += 1);
            }) as Box<dyn FnOnce()>);

            web_sys::window()
                .expect("no window")
                .request_animation_frame(closure.as_ref().unchecked_ref())
                .expect("rAF failed");
            closure.forget();
        }
    });
}

// Get current render version (for e2e tests)
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn get_render_version() -> u32 {
    TEST_STATE.with(|s| {
        s.borrow()
            .as_ref()
            .map(|state| state.render_version.get_untracked())
            .unwrap_or(0)
    })
}

// Check if doom chunk has any non-zero pixel data (for e2e tests).
// Returns the count of non-zero bytes in the doom chunk, or 0 if not loaded.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn get_doom_chunk_nonzero_count() -> u32 {
    use crate::utils::chunk_coords_to_id;
    let doom_chunk_id = chunk_coords_to_id(5, 5);
    TEST_STATE.with(|s| {
        s.borrow()
            .as_ref()
            .map(|state| {
                state.loaded_chunks.with_untracked(|chunks| {
                    chunks
                        .get(&doom_chunk_id)
                        .map(|data| data.iter().filter(|&&b| b != 0).count() as u32)
                        .unwrap_or(0)
                })
            })
            .unwrap_or(0)
    })
}

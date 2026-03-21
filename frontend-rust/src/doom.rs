//! Doom Mode - renders Doom gameplay as checkboxes
//!
//! This module integrates js-dos (Doom in browser) with the checkbox grid,
//! capturing frames and rendering them as binary checkbox patterns at chunk (5,5).

use crate::state::AppState;
use crate::utils::chunk_coords_to_id;
use leptos::prelude::*;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

// Constants matching jacobenget/doom.wasm (2x original Doom resolution)
const DOOM_WIDTH: u32 = 640;
const DOOM_HEIGHT: u32 = 400;
const CHUNK_OFFSET_X: i32 = 5000;
const CHUNK_OFFSET_Y: i32 = 5000;
const CHUNK_SIZE: u32 = 1000;

// How many doom frames to accumulate before flushing to SpacetimeDB
const BATCH_FRAMES: u32 = 3;

// Thread-local storage for the callback closure
thread_local! {
    static FRAME_CALLBACK: RefCell<Option<Closure<dyn FnMut(js_sys::Uint32Array, js_sys::Uint8Array, u32, u32, i32, i32)>>> = const { RefCell::new(None) };
    static DOOM_STATE: RefCell<Option<AppState>> = const { RefCell::new(None) };
    static FRAME_COUNTER: RefCell<u32> = const { RefCell::new(0) };
    static FPS_FRAME_COUNT: RefCell<u32> = const { RefCell::new(0) };
    static FPS_LAST_LOG: RefCell<f64> = const { RefCell::new(0.0) };
}

/// JavaScript bindings for DoomMode
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

/// Check if DoomMode JS is available
fn is_doom_available() -> bool {
    let window = web_sys::window().expect("no window");
    js_sys::Reflect::has(&window, &JsValue::from_str("DoomMode")).unwrap_or(false)
}

/// Show or hide the doom container
fn set_doom_container_visible(visible: bool) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(container) = document.get_element_by_id("doom-container") {
                let style = container
                    .dyn_ref::<web_sys::HtmlElement>()
                    .and_then(|el| Some(el.style()));
                if let Some(style) = style {
                    let _ = style.set_property("display", if visible { "block" } else { "none" });
                }
            }
        }
    }
}

/// Clear the Doom rendering area (chunk at offset 5, 5)
fn clear_doom_chunks(state: &AppState) {
    use crate::constants::CHUNK_DATA_SIZE;

    web_sys::console::log_1(&"Clearing Doom chunk...".into());

    // Calculate the chunk ID for the Doom area
    let chunk_x = CHUNK_OFFSET_X.div_euclid(CHUNK_SIZE as i32);
    let chunk_y = CHUNK_OFFSET_Y.div_euclid(CHUNK_SIZE as i32);
    let chunk_id = chunk_coords_to_id(chunk_x, chunk_y);

    // Pre-populate loaded_chunks with an empty chunk so the canvas renders
    // the doom area immediately (black grid) without waiting for SpacetimeDB
    state.loaded_chunks.update(|chunks| {
        chunks
            .entry(chunk_id)
            .or_insert_with(|| vec![0u8; CHUNK_DATA_SIZE]);
    });
    state.render_version.update(|v| *v += 1);

    // Create updates to clear all pixels in the Doom area
    let mut updates = Vec::new();

    for y in 0..DOOM_HEIGHT {
        for x in 0..DOOM_WIDTH {
            // Calculate local position within chunk
            let local_x = (CHUNK_OFFSET_X as u32 + x) % CHUNK_SIZE;
            let local_y = (CHUNK_OFFSET_Y as u32 + y) % CHUNK_SIZE;
            let cell_offset = local_y * CHUNK_SIZE + local_x;

            // Clear the checkbox (black, unchecked)
            updates.push((chunk_id, cell_offset, 0u8, 0u8, 0u8, false));
        }
    }

    web_sys::console::log_1(&format!("Clearing {} checkboxes in Doom area", updates.len()).into());

    // Send all updates to server
    state.pending_updates.update(|pending| {
        pending.extend(updates);
    });

    crate::db::flush_pending_updates(state.clone());

    web_sys::console::log_1(&"Doom chunk cleared and synced to database".into());
}

/// Initialize and start Doom mode
pub async fn start_doom_mode(state: AppState) -> Result<(), String> {
    web_sys::console::log_1(&"Checking DoomMode availability...".into());

    if !is_doom_available() {
        web_sys::console::error_1(&"DoomMode JavaScript object not found!".into());
        web_sys::console::log_1(&"Check if doom-jacobenget.js and doom-mode.js loaded correctly".into());
        return Err("DoomMode not available - check console for details".to_string());
    }

    web_sys::console::log_1(&"Starting Doom mode...".into());

    // Clear the Doom rendering area first
    clear_doom_chunks(&state);

    // Store state for the callback
    DOOM_STATE.with(|s| {
        *s.borrow_mut() = Some(state);
    });

    // Show the doom container
    set_doom_container_visible(true);

    // Initialize WASM Doom
    let promise = init("doom-container");
    let result = wasm_bindgen_futures::JsFuture::from(promise).await;

    match result {
        Ok(_) => {
            web_sys::console::log_1(&"Doom initialized, starting frame capture...".into());

            // Create the frame callback for delta updates with RGB colors
            let callback = Closure::wrap(Box::new(
                move |indices: js_sys::Uint32Array, color_data: js_sys::Uint8Array, width: u32, height: u32, offset_x: i32, offset_y: i32| {
                    handle_doom_frame_delta(indices, color_data, width, height, offset_x, offset_y);
                },
            )
                as Box<dyn FnMut(js_sys::Uint32Array, js_sys::Uint8Array, u32, u32, i32, i32)>);

            // Start capture
            startCapture(&callback);

            // Store the callback to prevent it from being dropped
            FRAME_CALLBACK.with(|c| {
                *c.borrow_mut() = Some(callback);
            });

            Ok(())
        }
        Err(e) => {
            set_doom_container_visible(false);
            Err(format!("Failed to initialize Doom: {:?}", e))
        }
    }
}

/// Stop Doom mode
pub fn stop_doom_mode() {
    web_sys::console::log_1(&"Stopping Doom mode...".into());

    if is_doom_available() {
        stopCapture();
        stop();
    }

    // Hide the container
    set_doom_container_visible(false);

    // Clean up the callback
    FRAME_CALLBACK.with(|c| {
        *c.borrow_mut() = None;
    });

    // Clear state
    DOOM_STATE.with(|s| {
        *s.borrow_mut() = None;
    });
}

/// Returns true if the given chunk_id is one of the chunks doom renders into.
/// SpacetimeDB updates for these chunks should be ignored while doom is running
/// to avoid overwriting optimistic frames with stale server data.
pub fn is_doom_chunk(chunk_id: i64) -> bool {
    use crate::utils::chunk_coords_to_id;
    // Doom renders into the chunk(s) covered by its pixel area.
    // With CHUNK_OFFSET (5000,5000) and size 640x400, all pixels land in chunk (5,5).
    let min_cx = CHUNK_OFFSET_X.div_euclid(CHUNK_SIZE as i32);
    let max_cx = (CHUNK_OFFSET_X + DOOM_WIDTH as i32 - 1).div_euclid(CHUNK_SIZE as i32);
    let min_cy = CHUNK_OFFSET_Y.div_euclid(CHUNK_SIZE as i32);
    let max_cy = (CHUNK_OFFSET_Y + DOOM_HEIGHT as i32 - 1).div_euclid(CHUNK_SIZE as i32);
    for cx in min_cx..=max_cx {
        for cy in min_cy..=max_cy {
            if chunk_coords_to_id(cx, cy) == chunk_id {
                return true;
            }
        }
    }
    false
}

/// Check if Doom mode is currently running
pub fn is_doom_running() -> bool {
    if is_doom_available() {
        isRunning()
    } else {
        false
    }
}

/// Toggle Doom controls (focus/unfocus the Doom canvas for keyboard input)
pub fn toggle_doom_controls() {
    if is_doom_available() {
        toggleControls();
    }
}

/// Enable Doom controls (focus the Doom canvas)
pub fn enable_doom_controls() {
    if is_doom_available() {
        enableControls();
    }
}

/// Handle a delta frame from Doom - only changed pixels with full RGB colors
fn handle_doom_frame_delta(
    indices: js_sys::Uint32Array,
    color_data: js_sys::Uint8Array,
    width: u32,
    _height: u32,
    offset_x: i32,
    offset_y: i32,
) {
    // FPS tracking
    let now = js_sys::Date::now();
    FPS_FRAME_COUNT.with(|c| *c.borrow_mut() += 1);
    FPS_LAST_LOG.with(|last| {
        let prev = *last.borrow();
        if prev == 0.0 {
            *last.borrow_mut() = now;
        } else if now - prev >= 1000.0 {
            let frames = FPS_FRAME_COUNT.with(|c| {
                let f = *c.borrow();
                *c.borrow_mut() = 0;
                f
            });
            let elapsed = (now - prev) / 1000.0;
            let pixels = indices.length();
            web_sys::console::log_1(&format!(
                "[PERF doom] {:.1} FPS ({} frames in {:.1}s) | {} changed pixels/frame",
                frames as f64 / elapsed, frames, elapsed, pixels
            ).into());
            *last.borrow_mut() = now;
        }
    });

    // Get state
    let state = DOOM_STATE.with(|s| s.borrow().clone());
    let Some(state) = state else {
        return;
    };

    let t0 = js_sys::Date::now();
    let pixel_count = indices.length() as usize;

    // Copy JS arrays to Rust once
    let indices_vec: Vec<u32> = indices.to_vec();
    let color_vec: Vec<u8> = color_data.to_vec();

    if indices_vec.is_empty() {
        return;
    }

    let t1 = js_sys::Date::now();

    // All Doom pixels land in a single chunk — precompute once.
    let base_local_x = offset_x.rem_euclid(CHUNK_SIZE as i32) as u32;
    let base_local_y = offset_y.rem_euclid(CHUNK_SIZE as i32) as u32;
    let chunk_x = offset_x.div_euclid(CHUNK_SIZE as i32);
    let chunk_y = offset_y.div_euclid(CHUNK_SIZE as i32);
    let chunk_id = chunk_coords_to_id(chunk_x, chunk_y);

    // Optimistic render only — write directly into chunk byte array
    state.loaded_chunks.update(|chunks| {
        use crate::constants::CHUNK_DATA_SIZE;
        let data = chunks.entry(chunk_id).or_insert_with(|| vec![0u8; CHUNK_DATA_SIZE]);

        for (i, &pixel_idx) in indices_vec.iter().enumerate() {
            let x = pixel_idx % width;
            let y = pixel_idx / width;
            let byte_idx = (((base_local_y + y) * CHUNK_SIZE + (base_local_x + x)) * 4) as usize;
            let color_idx = i * 4;

            if byte_idx + 3 < data.len() {
                data[byte_idx] = color_vec[color_idx];
                data[byte_idx + 1] = color_vec[color_idx + 1];
                data[byte_idx + 2] = color_vec[color_idx + 2];
                data[byte_idx + 3] = if color_vec[color_idx + 3] != 0 { 0xFF } else { 0x00 };
            }
        }
    });
    state.render_version.update(|v| *v += 1);

    let t2 = js_sys::Date::now();

    // Send raw doom frame data to worker — it handles packing + SpacetimeDB flush.
    // Set DOOM_SYNC_ENABLED to false to test FPS without syncing.
    const DOOM_SYNC_ENABLED: bool = true;

    if DOOM_SYNC_ENABLED {
        // Binary format: [tag: u8 = 4] [width: u32] [base_local_x: u32] [base_local_y: u32]
        //                [chunk_id: i64] [indices: N×u32] [colors: N×4 u8]
        let header_len = 1 + 4 + 4 + 4 + 8;
        let total = header_len + pixel_count * 4 + pixel_count * 4;

        let mut buf = Vec::with_capacity(total);
        buf.push(4u8);
        buf.extend_from_slice(&width.to_le_bytes());
        buf.extend_from_slice(&base_local_x.to_le_bytes());
        buf.extend_from_slice(&base_local_y.to_le_bytes());
        buf.extend_from_slice(&chunk_id.to_le_bytes());
        for &idx in &indices_vec {
            buf.extend_from_slice(&idx.to_le_bytes());
        }
        buf.extend_from_slice(&color_vec);

        crate::worker_bridge::send_binary_to_worker(&buf);
    }

    let t3 = js_sys::Date::now();
    if pixel_count > 1000 {
        web_sys::console::log_1(&format!(
            "[PERF doom-frame] copy={:.0}ms render={:.0}ms transfer={:.0}ms | {} pixels",
            t1 - t0, t2 - t1, t3 - t2, pixel_count
        ).into());
    }
}

// Keep the old function for reference but it's no longer used
#[allow(dead_code)]
/// Handle a frame from Doom - convert to checkbox updates and send to server
fn handle_doom_frame(
    data: js_sys::Uint8Array,
    width: u32,
    height: u32,
    offset_x: i32,
    offset_y: i32,
) {
    let t0 = js_sys::Date::now();
    
    // Get the checkbox data from JS
    let checkbox_data = data.to_vec();

    // Get state
    let state = DOOM_STATE.with(|s| s.borrow().clone());
    let Some(state) = state else {
        return;
    };

    let t1 = js_sys::Date::now();
    
    // Convert frame to batch updates
    // Each pixel becomes a checkbox at (offset_x + x, offset_y + y)
    let updates = frame_to_updates(&checkbox_data, width, height, offset_x, offset_y);

    if updates.is_empty() {
        return;
    }

    let t2 = js_sys::Date::now();

    // Send batch update to server via the existing mechanism
    // We'll add the updates to the pending queue and flush
    state.pending_updates.update(|pending| {
        pending.extend(updates);
    });

    let t3 = js_sys::Date::now();

    // Flush immediately for Doom frames
    crate::db::flush_pending_updates(state);
    
    let t4 = js_sys::Date::now();
    
    web_sys::console::log_1(&format!(
        "Rust timing: copy={:.0}ms, convert={:.0}ms, signal={:.0}ms, flush={:.0}ms, total={:.0}ms",
        t1 - t0, t2 - t1, t3 - t2, t4 - t3, t4 - t0
    ).into());
}

/// Convert a binary frame to checkbox updates
/// Returns Vec of (chunk_id, cell_offset, r, g, b, checked)
fn frame_to_updates(
    data: &[u8],
    width: u32,
    height: u32,
    offset_x: i32,
    offset_y: i32,
) -> Vec<(i64, u32, u8, u8, u8, bool)> {
    let mut updates = Vec::with_capacity((width * height) as usize);

    // Doom green color for checked checkboxes
    let (r, g, b) = (0, 255, 0);

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let checked = data.get(idx).copied().unwrap_or(0) != 0;

            // Calculate global grid position
            let grid_x = offset_x + x as i32;
            let grid_y = offset_y + y as i32;

            // Calculate chunk coordinates
            let chunk_x = grid_x.div_euclid(CHUNK_SIZE as i32);
            let chunk_y = grid_y.div_euclid(CHUNK_SIZE as i32);
            let chunk_id = chunk_coords_to_id(chunk_x, chunk_y);

            // Calculate local position within chunk
            let local_x = grid_x.rem_euclid(CHUNK_SIZE as i32) as u32;
            let local_y = grid_y.rem_euclid(CHUNK_SIZE as i32) as u32;
            let cell_offset = local_y * CHUNK_SIZE + local_x;

            updates.push((chunk_id, cell_offset, r, g, b, checked));
        }
    }

    updates
}

/// Navigate viewport to the Doom location
pub fn go_to_doom_location(state: AppState) {
    use crate::constants::CELL_SIZE;

    // Get canvas size
    let (canvas_w, canvas_h) = if let Some(window) = web_sys::window() {
        let w = window
            .inner_width()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(1200.0)
            - 40.0;
        let h = window
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(800.0)
            - 120.0;
        (w, h)
    } else {
        (1200.0, 800.0)
    };

    // Center on Doom area (middle of the 320x200 region at chunk 5,5)
    let doom_center_x = CHUNK_OFFSET_X as f64 + (DOOM_WIDTH as f64 / 2.0);
    let doom_center_y = CHUNK_OFFSET_Y as f64 + (DOOM_HEIGHT as f64 / 2.0);

    // Set scale to fit Doom in view (with some padding)
    let scale_x = canvas_w / (DOOM_WIDTH as f64 * CELL_SIZE * 1.2);
    let scale_y = canvas_h / (DOOM_HEIGHT as f64 * CELL_SIZE * 1.2);
    let scale = scale_x.min(scale_y).min(2.0).max(0.5);

    let cell_size = CELL_SIZE * scale;

    // Calculate offset to center Doom
    let offset_x = canvas_w / 2.0 - doom_center_x * cell_size;
    let offset_y = canvas_h / 2.0 - doom_center_y * cell_size;

    state.offset_x.set(offset_x);
    state.offset_y.set(offset_y);
    state.scale.set(scale);

    web_sys::console::log_1(
        &format!(
            "Navigated to Doom location: ({}, {}) scale={}",
            doom_center_x, doom_center_y, scale
        )
        .into(),
    );
}

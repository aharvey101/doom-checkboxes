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

// Constants matching original Doom resolution
const DOOM_WIDTH: u32 = 320;
const DOOM_HEIGHT: u32 = 200;
const CHUNK_OFFSET_X: i32 = 5000;
const CHUNK_OFFSET_Y: i32 = 5000;
const CHUNK_SIZE: u32 = 1000;

// Thread-local storage for the callback closure
thread_local! {
    static FRAME_CALLBACK: RefCell<Option<Closure<dyn FnMut(js_sys::Uint32Array, js_sys::Uint8Array, u32, u32, i32, i32)>>> = const { RefCell::new(None) };
    static DOOM_STATE: RefCell<Option<AppState>> = const { RefCell::new(None) };
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
    web_sys::console::log_1(&"Clearing Doom chunk...".into());

    // Calculate the chunk ID for the Doom area
    let chunk_x = CHUNK_OFFSET_X.div_euclid(CHUNK_SIZE as i32);
    let chunk_y = CHUNK_OFFSET_Y.div_euclid(CHUNK_SIZE as i32);
    let chunk_id = chunk_coords_to_id(chunk_x, chunk_y);

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

    web_sys::console::log_1(&"Doom chunk cleared".into());
}

/// Initialize and start Doom mode
pub async fn start_doom_mode(state: AppState) -> Result<(), String> {
    if !is_doom_available() {
        return Err("DoomMode not available - js-dos not loaded".to_string());
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

    // Initialize js-dos
    let promise = init("doom-container");
    let result = wasm_bindgen_futures::JsFuture::from(promise).await;

    match result {
        Ok(_) => {
            web_sys::console::log_1(&"Doom initialized, starting frame capture...".into());

            // Create the frame callback for delta updates
            let callback = Closure::wrap(Box::new(
                move |indices: js_sys::Uint32Array, values: js_sys::Uint8Array, width: u32, height: u32, offset_x: i32, offset_y: i32| {
                    handle_doom_frame_delta(indices, values, width, height, offset_x, offset_y);
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

/// Handle a delta frame from Doom - only changed pixels
fn handle_doom_frame_delta(
    indices: js_sys::Uint32Array,
    values: js_sys::Uint8Array,
    width: u32,
    _height: u32,
    offset_x: i32,
    offset_y: i32,
) {
    let t0 = js_sys::Date::now();
    
    // Get state
    let state = DOOM_STATE.with(|s| s.borrow().clone());
    let Some(state) = state else {
        return;
    };

    // Convert delta to updates
    let indices_vec: Vec<u32> = indices.to_vec();
    let values_vec: Vec<u8> = values.to_vec();
    
    let t1 = js_sys::Date::now();
    
    let mut updates = Vec::with_capacity(indices_vec.len());
    let (r, g, b) = (0, 255, 0); // Doom green
    
    for (i, &pixel_idx) in indices_vec.iter().enumerate() {
        let checked = values_vec.get(i).copied().unwrap_or(0) != 0;
        
        // Convert linear index to x,y
        let x = (pixel_idx % width) as i32;
        let y = (pixel_idx / width) as i32;
        
        // Calculate global grid position
        let grid_x = offset_x + x;
        let grid_y = offset_y + y;
        
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

    let t2 = js_sys::Date::now();

    if updates.is_empty() {
        return;
    }

    // Send batch update to server
    // OPTIMIZATION: Apply updates locally only (optimistic)
    // Don't add to pending_updates or flush to server
    // This eliminates the 24-32 MB server broadcast bottleneck
    state.loaded_chunks.update(|chunks| {
        for (chunk_id, cell_offset, r, g, b, checked) in &updates {
            chunks.entry(*chunk_id).or_insert_with(|| vec![0u8; crate::constants::CHUNK_DATA_SIZE]);
            if let Some(data) = chunks.get_mut(chunk_id) {
                crate::db::set_checkbox(data, *cell_offset as usize, *r, *g, *b, *checked);
            }
        }
    });

    let t3 = js_sys::Date::now();

    // Trigger re-render
    state.render_version.update(|v| *v += 1);

    let t4 = js_sys::Date::now();

    web_sys::console::log_1(&format!(
        "Rust delta: {} changes, local_update={:.0}ms, render={:.0}ms, total={:.0}ms (NO SERVER)",
        indices_vec.len(), t3 - t2, t4 - t3, t4 - t0
    ).into());
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

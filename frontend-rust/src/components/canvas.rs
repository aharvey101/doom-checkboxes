use leptos::html::Canvas;
use leptos::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MouseEvent, WheelEvent};

use crate::constants::*;
use crate::db::{set_checkbox_checked, subscribe_to_chunks, toggle_checkbox};
use crate::state::AppState;
use crate::utils::{canvas_to_grid, visible_chunk_ids};
use crate::webgl::WebGLRenderer;

#[component]
pub fn CheckboxCanvas(state: AppState) -> impl IntoView {
    let canvas_ref = NodeRef::<Canvas>::new();

    // WebGL renderer stored in RefCell for mutation
    let renderer: Rc<RefCell<Option<WebGLRenderer>>> = Rc::new(RefCell::new(None));

    // Request a render on the next animation frame (throttled)
    let request_render = {
        let renderer_clone = renderer.clone();
        let canvas_ref_clone = canvas_ref;
        move || {
            // If render already pending, skip
            if state.render_pending.get_untracked() {
                return;
            }
            state.render_pending.set(true);

            let renderer_inner = renderer_clone.clone();
            let canvas_ref_inner = canvas_ref_clone;
            let state_copy = state;

            // Schedule render on next animation frame
            let closure = Closure::once(Box::new(move || {
                state_copy.render_pending.set(false);
                // Use get_untracked() since we're in a requestAnimationFrame callback,
                // not a reactive tracking context
                if let Some(canvas) = canvas_ref_inner.get_untracked() {
                    let mut renderer_borrow = renderer_inner.borrow_mut();

                    // Initialize WebGL renderer on first render
                    if renderer_borrow.is_none() {
                        match WebGLRenderer::new(&canvas) {
                            Ok(r) => {
                                web_sys::console::log_1(&"WebGL renderer initialized".into());
                                *renderer_borrow = Some(r);
                            }
                            Err(e) => {
                                web_sys::console::error_1(
                                    &format!("WebGL init failed: {}", e).into(),
                                );
                                return;
                            }
                        }
                    }

                    if let Some(ref r) = *renderer_borrow {
                        let loaded_chunks = state_copy.loaded_chunks.get_untracked();
                        let offset_x = state_copy.offset_x.get_untracked();
                        let offset_y = state_copy.offset_y.get_untracked();
                        let scale = state_copy.scale.get_untracked();
                        r.render(&canvas, &loaded_chunks, offset_x, offset_y, scale);
                    }
                }
            }) as Box<dyn FnOnce()>);

            web_sys::window()
                .expect("no window")
                .request_animation_frame(closure.as_ref().unchecked_ref())
                .expect("failed to request animation frame");

            // Prevent closure from being dropped
            closure.forget();
        }
    };

    // Render effect for viewport changes and chunk updates
    let request_render_effect = request_render.clone();
    Effect::new(move |_| {
        // Track viewport changes
        let _ = state.offset_x.get();
        let _ = state.offset_y.get();
        let _ = state.scale.get();
        // Track chunk data changes
        let _ = state.loaded_chunks.get();
        // Track server updates (incremented when server sends new data)
        let _ = state.render_version.get();

        request_render_effect();
    });

    // Chunk subscription effect - subscribe to visible chunks when viewport changes
    // Only subscribe when connected to avoid "Still in CONNECTING state" errors
    Effect::new(move |_| {
        // Track connection status - only subscribe when connected
        let status = state.status.get();
        if status != crate::state::ConnectionStatus::Connected {
            return;
        }

        let offset_x = state.offset_x.get();
        let offset_y = state.offset_y.get();
        let scale = state.scale.get();

        // Use get_untracked to avoid reactive tracking warning for canvas ref
        if let Some(canvas) = canvas_ref.get_untracked() {
            let width = canvas.width() as f64;
            let height = canvas.height() as f64;

            let visible = visible_chunk_ids(offset_x, offset_y, scale, width, height);
            subscribe_to_chunks(state, visible);
        }
    });

    // Window resize effect
    let renderer_for_resize = renderer.clone();
    Effect::new(move |_| {
        if let Some(canvas) = canvas_ref.get() {
            let window = web_sys::window().expect("no window");
            let width = window.inner_width().unwrap().as_f64().unwrap() - 40.0;
            let height = window.inner_height().unwrap().as_f64().unwrap() - 120.0;
            canvas.set_width(width as u32);
            canvas.set_height(height as u32);

            // Notify renderer of resize
            if let Some(ref r) = *renderer_for_resize.borrow() {
                r.resize(width as u32, height as u32);
            }

            // Trigger re-render
            state.render_pending.set(false); // Reset to allow new render
        }
    });

    // Click handler - with immediate visual feedback
    let renderer_for_click = renderer.clone();
    let on_click = move |e: MouseEvent| {
        // Don't handle click if we were drawing (mouseup already handled it)
        if state.is_drawing.get_untracked() {
            return;
        }
        if e.shift_key() {
            return; // Shift+click is for panning
        }

        let Some(canvas) = canvas_ref.get() else {
            return;
        };
        let rect = canvas.get_bounding_client_rect();
        let x = e.client_x() as f64 - rect.left();
        let y = e.client_y() as f64 - rect.top();

        let offset_x = state.offset_x.get_untracked();
        let offset_y = state.offset_y.get_untracked();
        let scale = state.scale.get_untracked();

        if let Some((col, row)) = canvas_to_grid(x, y, offset_x, offset_y, scale) {
            // Toggle and get new value
            if let Some(new_value) = toggle_checkbox(state, col, row) {
                // Immediate visual feedback - render just this cell
                if let Some(ref r) = *renderer_for_click.borrow() {
                    r.render_cell_immediate(
                        &canvas, col, row, new_value, offset_x, offset_y, scale,
                    );
                }
            }
        }
    };

    // Pan handlers (shift+drag) and drawing handlers (drag without shift)
    let renderer_for_draw = renderer.clone();
    let on_mousedown = move |e: MouseEvent| {
        if e.button() == 0 {
            if e.shift_key() {
                // Shift+drag: pan mode
                state.is_dragging.set(true);
                state.last_mouse_x.set(e.client_x() as f64);
                state.last_mouse_y.set(e.client_y() as f64);
            } else {
                // Regular drag: drawing mode
                state.is_drawing.set(true);

                // Fill the first checkbox immediately
                let Some(canvas) = canvas_ref.get() else {
                    return;
                };
                let rect = canvas.get_bounding_client_rect();
                let x = e.client_x() as f64 - rect.left();
                let y = e.client_y() as f64 - rect.top();

                let offset_x = state.offset_x.get_untracked();
                let offset_y = state.offset_y.get_untracked();
                let scale = state.scale.get_untracked();

                if let Some((col, row)) = canvas_to_grid(x, y, offset_x, offset_y, scale) {
                    if let Some(true) = set_checkbox_checked(state, col, row) {
                        // Render immediately
                        if let Some(ref r) = *renderer_for_draw.borrow() {
                            r.render_cell_immediate(
                                &canvas, col, row, true, offset_x, offset_y, scale,
                            );
                        }
                    }
                }
            }
        }
    };

    let renderer_for_move = renderer.clone();
    let on_mousemove = move |e: MouseEvent| {
        if state.is_dragging.get() {
            // Pan mode
            let dx = e.client_x() as f64 - state.last_mouse_x.get();
            let dy = e.client_y() as f64 - state.last_mouse_y.get();
            state.offset_x.update(|x| *x += dx);
            state.offset_y.update(|y| *y += dy);
            state.last_mouse_x.set(e.client_x() as f64);
            state.last_mouse_y.set(e.client_y() as f64);
        } else if state.is_drawing.get() {
            // Drawing mode - fill checkboxes under pointer
            let Some(canvas) = canvas_ref.get() else {
                return;
            };
            let rect = canvas.get_bounding_client_rect();
            let x = e.client_x() as f64 - rect.left();
            let y = e.client_y() as f64 - rect.top();

            let offset_x = state.offset_x.get_untracked();
            let offset_y = state.offset_y.get_untracked();
            let scale = state.scale.get_untracked();

            if let Some((col, row)) = canvas_to_grid(x, y, offset_x, offset_y, scale) {
                if let Some(true) = set_checkbox_checked(state, col, row) {
                    // Render immediately
                    if let Some(ref r) = *renderer_for_move.borrow() {
                        r.render_cell_immediate(&canvas, col, row, true, offset_x, offset_y, scale);
                    }
                }
            }
        }
    };

    let on_mouseup = move |_: MouseEvent| {
        state.is_dragging.set(false);
        state.is_drawing.set(false);
    };

    let on_mouseleave = move |_: MouseEvent| {
        state.is_dragging.set(false);
        state.is_drawing.set(false);
    };

    // Zoom handler
    let on_wheel = move |e: WheelEvent| {
        e.prevent_default();
        handle_wheel(e, &state, &canvas_ref);
    };

    let cursor_style = move || {
        if state.is_dragging.get() {
            "cursor: grabbing"
        } else {
            "cursor: crosshair"
        }
    };

    view! {
        <canvas
            node_ref=canvas_ref
            on:click=on_click
            on:mousedown=on_mousedown
            on:mousemove=on_mousemove
            on:mouseup=on_mouseup
            on:mouseleave=on_mouseleave
            on:wheel=on_wheel
            style=cursor_style
        />
    }
}

fn handle_wheel(e: WheelEvent, state: &AppState, canvas_ref: &NodeRef<Canvas>) {
    let Some(canvas) = canvas_ref.get() else {
        return;
    };
    let rect = canvas.get_bounding_client_rect();
    let mouse_x = e.client_x() as f64 - rect.left();
    let mouse_y = e.client_y() as f64 - rect.top();

    let current_scale = state.scale.get();
    let zoom_factor = if e.delta_y() > 0.0 { 0.9 } else { 1.1 };
    let new_scale = (current_scale * zoom_factor).clamp(MIN_SCALE, MAX_SCALE);

    // Zoom toward mouse position
    let scale_change = new_scale / current_scale;
    let offset_x = state.offset_x.get();
    let offset_y = state.offset_y.get();

    state
        .offset_x
        .set(mouse_x - (mouse_x - offset_x) * scale_change);
    state
        .offset_y
        .set(mouse_y - (mouse_y - offset_y) * scale_change);
    state.scale.set(new_scale);
}

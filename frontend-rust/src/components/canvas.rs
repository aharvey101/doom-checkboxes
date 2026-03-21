use leptos::html::Canvas;
use leptos::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MouseEvent, TouchEvent, WheelEvent};

use crate::constants::*;
use crate::db::{
    flush_pending_updates, set_checkbox_checked, set_checkbox_unchecked, subscribe_to_chunks,
    toggle_checkbox,
};
use crate::state::AppState;
use crate::utils::{canvas_to_grid, visible_chunks};
use crate::webgl::WebGLRenderer;

/// Calculate distance between two touch points
fn touch_distance(t1: &web_sys::Touch, t2: &web_sys::Touch) -> f64 {
    let dx = t1.client_x() as f64 - t2.client_x() as f64;
    let dy = t1.client_y() as f64 - t2.client_y() as f64;
    (dx * dx + dy * dy).sqrt()
}

/// Calculate midpoint between two touch points
fn touch_midpoint(t1: &web_sys::Touch, t2: &web_sys::Touch) -> (f64, f64) {
    let x = (t1.client_x() as f64 + t2.client_x() as f64) / 2.0;
    let y = (t1.client_y() as f64 + t2.client_y() as f64) / 2.0;
    (x, y)
}

/// Bresenham's line algorithm - returns all grid cells between two points (inclusive)
/// Uses signed coordinates for infinite grid
fn line_cells(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let mut cells = Vec::new();

    let dx = (x1 as i64 - x0 as i64).abs();
    let dy = (y1 as i64 - y0 as i64).abs();
    let sx: i64 = if x0 < x1 { 1 } else { -1 };
    let sy: i64 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    let mut x = x0 as i64;
    let mut y = y0 as i64;

    loop {
        cells.push((x as i32, y as i32));

        if x == x1 as i64 && y == y1 as i64 {
            break;
        }

        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }

    cells
}

/// Get all grid cells within a circle of given radius (in pixels) around a center point
/// Returns cells within the brush area
fn cells_in_radius(center_col: i32, center_row: i32, radius_pixels: f64, scale: f64) -> Vec<(i32, i32)> {
    use crate::constants::CELL_SIZE;

    if radius_pixels <= 0.0 {
        return vec![(center_col, center_row)];
    }

    let cell_size = CELL_SIZE * scale;
    let radius_cells = (radius_pixels / cell_size).ceil() as i32 + 1;

    let mut cells = Vec::new();
    let radius_sq = radius_pixels * radius_pixels;

    for dy in -radius_cells..=radius_cells {
        for dx in -radius_cells..=radius_cells {
            let col = center_col + dx;
            let row = center_row + dy;

            // Calculate distance in pixels from center
            let pixel_dx = dx as f64 * cell_size;
            let pixel_dy = dy as f64 * cell_size;
            let dist_sq = pixel_dx * pixel_dx + pixel_dy * pixel_dy;

            if dist_sq <= radius_sq {
                cells.push((col, row));
            }
        }
    }

    cells
}

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
                        let offset_x = state_copy.offset_x.get_untracked();
                        let offset_y = state_copy.offset_y.get_untracked();
                        let scale = state_copy.scale.get_untracked();
                        // Use with_untracked to avoid cloning the entire HashMap (36MB+)
                        state_copy.loaded_chunks.with_untracked(|loaded_chunks| {
                            r.render(&canvas, loaded_chunks, offset_x, offset_y, scale);
                        });
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
        // Track server updates and chunk changes via render_version
        // (do NOT track loaded_chunks.get() here — it clones the entire HashMap
        // which contains 4MB+ Vec<u8> per chunk, causing massive allocations on every frame)
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

            let visible = visible_chunks(offset_x, offset_y, scale, width, height);
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

        let (col, row) = canvas_to_grid(x, y, offset_x, offset_y, scale);
        // Toggle and get new value
        if let Some(new_value) = toggle_checkbox(state, col, row) {
            // Immediate visual feedback - render just this cell
            if let Some(ref r) = *renderer_for_click.borrow() {
                let user_color = state.user_color.get_untracked();
                r.render_cell_immediate(
                    &canvas, col, row, new_value, user_color, offset_x, offset_y, scale,
                );
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

                let (col, row) = canvas_to_grid(x, y, offset_x, offset_y, scale);
                // Track the starting position for line interpolation
                state.last_draw_col.set(Some(col));
                state.last_draw_row.set(Some(row));

                // Check eraser mode and brush size
                let eraser_mode = state.eraser_mode.get_untracked();
                let brush_size = state.brush_size.get_untracked();

                // Get all cells in brush radius
                let cells = cells_in_radius(col, row, brush_size, scale);

                // Fill or erase all cells in brush
                for (c, r) in cells {
                    let changed = if eraser_mode {
                        set_checkbox_unchecked(state, c, r)
                    } else {
                        set_checkbox_checked(state, c, r)
                    };

                    if let Some(true) = changed {
                        // Render immediately
                        if let Some(ref renderer) = *renderer_for_draw.borrow() {
                            let user_color = state.user_color.get_untracked();
                            renderer.render_cell_immediate(
                                &canvas,
                                c,
                                r,
                                !eraser_mode,
                                user_color,
                                offset_x,
                                offset_y,
                                scale,
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
            // Drawing mode - fill checkboxes along line from last position
            let Some(canvas) = canvas_ref.get() else {
                return;
            };
            let rect = canvas.get_bounding_client_rect();
            let x = e.client_x() as f64 - rect.left();
            let y = e.client_y() as f64 - rect.top();

            let offset_x = state.offset_x.get_untracked();
            let offset_y = state.offset_y.get_untracked();
            let scale = state.scale.get_untracked();

            let (col, row) = canvas_to_grid(x, y, offset_x, offset_y, scale);
            // Get last position for line interpolation
            let last_col = state.last_draw_col.get_untracked();
            let last_row = state.last_draw_row.get_untracked();

            // Get all cells along the line from last position to current
            let line_points = match (last_col, last_row) {
                (Some(lc), Some(lr)) if lc != col || lr != row => line_cells(lc, lr, col, row),
                _ => vec![(col, row)],
            };

            // Get brush parameters
            let eraser_mode = state.eraser_mode.get_untracked();
            let brush_size = state.brush_size.get_untracked();

            // For each point along the line, apply the brush
            for (line_col, line_row) in line_points {
                let cells = cells_in_radius(line_col, line_row, brush_size, scale);

                for (c, r) in cells {
                    let changed = if eraser_mode {
                        set_checkbox_unchecked(state, c, r)
                    } else {
                        set_checkbox_checked(state, c, r)
                    };

                    if let Some(true) = changed {
                        // Render immediately
                        if let Some(ref renderer) = *renderer_for_move.borrow() {
                            let user_color = state.user_color.get_untracked();
                            renderer.render_cell_immediate(
                                &canvas,
                                c,
                                r,
                                !eraser_mode,
                                user_color,
                                offset_x,
                                offset_y,
                                scale,
                            );
                        }
                    }
                }
            }

            // Update last position
            state.last_draw_col.set(Some(col));
            state.last_draw_row.set(Some(row));
        }
    };

    let on_mouseup = move |_: MouseEvent| {
        // Flush any pending updates when drawing stops
        if state.is_drawing.get_untracked() {
            flush_pending_updates(state);
        }
        state.is_dragging.set(false);
        state.is_drawing.set(false);
        state.last_draw_col.set(None);
        state.last_draw_row.set(None);
    };

    let on_mouseleave = move |_: MouseEvent| {
        // Flush any pending updates when mouse leaves canvas
        if state.is_drawing.get_untracked() {
            flush_pending_updates(state);
        }
        state.is_dragging.set(false);
        state.is_drawing.set(false);
        state.last_draw_col.set(None);
        state.last_draw_row.set(None);
    };

    // Zoom handler
    let on_wheel = move |e: WheelEvent| {
        e.prevent_default();
        handle_wheel(e, &state, &canvas_ref);
    };

    // Touch handlers for mobile
    let renderer_for_touchstart = renderer.clone();
    let on_touchstart = move |e: TouchEvent| {
        e.prevent_default(); // Prevent scroll/zoom browser behavior

        let touches = e.touches();
        let touch_count = touches.length();
        state.touch_count.set(touch_count);

        if touch_count == 1 {
            // Single finger - start drawing mode
            let touch = touches.get(0).unwrap();
            let Some(canvas) = canvas_ref.get() else {
                return;
            };
            let rect = canvas.get_bounding_client_rect();
            let x = touch.client_x() as f64 - rect.left();
            let y = touch.client_y() as f64 - rect.top();

            let offset_x = state.offset_x.get_untracked();
            let offset_y = state.offset_y.get_untracked();
            let scale = state.scale.get_untracked();

            let (col, row) = canvas_to_grid(x, y, offset_x, offset_y, scale);

            // Start drawing
            state.is_drawing.set(true);
            state.last_draw_col.set(Some(col));
            state.last_draw_row.set(Some(row));

            // Get brush parameters
            let eraser_mode = state.eraser_mode.get_untracked();
            let brush_size = state.brush_size.get_untracked();

            // Get all cells in brush radius
            let cells = cells_in_radius(col, row, brush_size, scale);

            // Fill or erase all cells in brush
            for (c, r) in cells {
                let changed = if eraser_mode {
                    set_checkbox_unchecked(state, c, r)
                } else {
                    set_checkbox_checked(state, c, r)
                };

                if let Some(true) = changed {
                    if let Some(ref renderer) = *renderer_for_touchstart.borrow() {
                        let user_color = state.user_color.get_untracked();
                        renderer.render_cell_immediate(
                            &canvas,
                            c,
                            r,
                            !eraser_mode,
                            user_color,
                            offset_x,
                            offset_y,
                            scale,
                        );
                    }
                }
            }
        } else if touch_count == 2 {
            // Two fingers - start pinch/pan mode
            state.is_drawing.set(false);
            state.is_pinching.set(true);

            let t1 = touches.get(0).unwrap();
            let t2 = touches.get(1).unwrap();

            state.last_touch_distance.set(touch_distance(&t1, &t2));
            state.last_touch_midpoint.set(touch_midpoint(&t1, &t2));
        }
    };

    let renderer_for_touchmove = renderer.clone();
    let on_touchmove = move |e: TouchEvent| {
        e.prevent_default();

        let touches = e.touches();
        let touch_count = touches.length();

        if touch_count == 1 && state.is_drawing.get_untracked() {
            // Single finger drawing
            let touch = touches.get(0).unwrap();
            let Some(canvas) = canvas_ref.get() else {
                return;
            };
            let rect = canvas.get_bounding_client_rect();
            let x = touch.client_x() as f64 - rect.left();
            let y = touch.client_y() as f64 - rect.top();

            let offset_x = state.offset_x.get_untracked();
            let offset_y = state.offset_y.get_untracked();
            let scale = state.scale.get_untracked();

            let (col, row) = canvas_to_grid(x, y, offset_x, offset_y, scale);
            let last_col = state.last_draw_col.get_untracked();
            let last_row = state.last_draw_row.get_untracked();

            // Get all cells along the line
            let line_points = match (last_col, last_row) {
                (Some(lc), Some(lr)) if lc != col || lr != row => line_cells(lc, lr, col, row),
                _ => vec![(col, row)],
            };

            // Get brush parameters
            let eraser_mode = state.eraser_mode.get_untracked();
            let brush_size = state.brush_size.get_untracked();

            // For each point along the line, apply the brush
            for (line_col, line_row) in line_points {
                let cells = cells_in_radius(line_col, line_row, brush_size, scale);

                for (c, r) in cells {
                    let changed = if eraser_mode {
                        set_checkbox_unchecked(state, c, r)
                    } else {
                        set_checkbox_checked(state, c, r)
                    };

                    if let Some(true) = changed {
                        if let Some(ref renderer) = *renderer_for_touchmove.borrow() {
                            let user_color = state.user_color.get_untracked();
                            renderer.render_cell_immediate(
                                &canvas,
                                c,
                                r,
                                !eraser_mode,
                                user_color,
                                offset_x,
                                offset_y,
                                scale,
                            );
                        }
                    }
                }
            }

            state.last_draw_col.set(Some(col));
            state.last_draw_row.set(Some(row));
        } else if touch_count == 2 && state.is_pinching.get_untracked() {
            // Two finger pinch/pan
            let t1 = touches.get(0).unwrap();
            let t2 = touches.get(1).unwrap();

            let new_distance = touch_distance(&t1, &t2);
            let new_midpoint = touch_midpoint(&t1, &t2);

            let last_distance = state.last_touch_distance.get_untracked();
            let (last_mid_x, last_mid_y) = state.last_touch_midpoint.get_untracked();

            // Handle zoom (pinch)
            if last_distance > 0.0 {
                let Some(canvas) = canvas_ref.get() else {
                    return;
                };
                let rect = canvas.get_bounding_client_rect();
                let zoom_center_x = new_midpoint.0 - rect.left();
                let zoom_center_y = new_midpoint.1 - rect.top();

                let scale_factor = new_distance / last_distance;
                let current_scale = state.scale.get_untracked();
                let new_scale = (current_scale * scale_factor).clamp(MIN_SCALE, MAX_SCALE);

                // Zoom toward pinch center
                let scale_change = new_scale / current_scale;
                let offset_x = state.offset_x.get_untracked();
                let offset_y = state.offset_y.get_untracked();

                state
                    .offset_x
                    .set(zoom_center_x - (zoom_center_x - offset_x) * scale_change);
                state
                    .offset_y
                    .set(zoom_center_y - (zoom_center_y - offset_y) * scale_change);
                state.scale.set(new_scale);
            }

            // Handle pan (two finger drag)
            let dx = new_midpoint.0 - last_mid_x;
            let dy = new_midpoint.1 - last_mid_y;
            state.offset_x.update(|x| *x += dx);
            state.offset_y.update(|y| *y += dy);

            state.last_touch_distance.set(new_distance);
            state.last_touch_midpoint.set(new_midpoint);
        }
    };

    let on_touchend = move |e: TouchEvent| {
        e.prevent_default();

        let remaining_touches = e.touches().length();
        state.touch_count.set(remaining_touches);

        if remaining_touches == 0 {
            // All fingers lifted - flush updates
            if state.is_drawing.get_untracked() {
                flush_pending_updates(state);
            }
            state.is_drawing.set(false);
            state.is_pinching.set(false);
            state.last_draw_col.set(None);
            state.last_draw_row.set(None);
        } else if remaining_touches == 1 && state.is_pinching.get_untracked() {
            // Went from 2 fingers to 1 - stop pinching, could start drawing
            state.is_pinching.set(false);
        }
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
            on:touchstart=on_touchstart
            on:touchmove=on_touchmove
            on:touchend=on_touchend
            on:touchcancel=on_touchend
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

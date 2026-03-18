use leptos::html::Canvas;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, MouseEvent, WheelEvent};

use crate::constants::*;
use crate::state::AppState;
use crate::utils::{canvas_to_grid, count_bits, get_bit, set_bit};

#[component]
pub fn CheckboxCanvas(state: AppState) -> impl IntoView {
    let canvas_ref = NodeRef::<Canvas>::new();

    // Render effect - re-renders when signals change
    Effect::new(move |_| {
        let _ = state.chunk_data.get();
        let _ = state.offset_x.get();
        let _ = state.offset_y.get();
        let _ = state.scale.get();

        if let Some(canvas) = canvas_ref.get() {
            render_grid(&canvas, &state);
        }
    });

    // Window resize effect
    Effect::new(move |_| {
        if let Some(canvas) = canvas_ref.get() {
            let window = web_sys::window().expect("no window");
            let width = window.inner_width().unwrap().as_f64().unwrap() - 40.0;
            let height = window.inner_height().unwrap().as_f64().unwrap() - 120.0;
            canvas.set_width(width as u32);
            canvas.set_height(height as u32);
            render_grid(&canvas, &state);
        }
    });

    // Click handler
    let on_click = move |e: MouseEvent| {
        if e.shift_key() {
            return; // Shift+click is for panning
        }
        handle_click(e, &state, &canvas_ref);
    };

    // Pan handlers
    let on_mousedown = move |e: MouseEvent| {
        if e.button() == 0 && e.shift_key() {
            state.is_dragging.set(true);
            state.last_mouse_x.set(e.client_x() as f64);
            state.last_mouse_y.set(e.client_y() as f64);
        }
    };

    let on_mousemove = move |e: MouseEvent| {
        if state.is_dragging.get() {
            let dx = e.client_x() as f64 - state.last_mouse_x.get();
            let dy = e.client_y() as f64 - state.last_mouse_y.get();
            state.offset_x.update(|x| *x += dx);
            state.offset_y.update(|y| *y += dy);
            state.last_mouse_x.set(e.client_x() as f64);
            state.last_mouse_y.set(e.client_y() as f64);
        }
    };

    let on_mouseup = move |_: MouseEvent| {
        state.is_dragging.set(false);
    };

    let on_mouseleave = move |_: MouseEvent| {
        state.is_dragging.set(false);
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

fn render_grid(canvas: &HtmlCanvasElement, state: &AppState) {
    let ctx: CanvasRenderingContext2d = canvas
        .get_context("2d")
        .unwrap()
        .unwrap()
        .dyn_into()
        .unwrap();

    let width = canvas.width() as f64;
    let height = canvas.height() as f64;

    let chunk_data = state.chunk_data.get();
    let offset_x = state.offset_x.get();
    let offset_y = state.offset_y.get();
    let scale = state.scale.get();
    let cell_size = CELL_SIZE * scale;

    // Clear canvas
    ctx.set_fill_style_str(COLOR_GRID);
    ctx.fill_rect(0.0, 0.0, width, height);

    // Calculate visible range
    let start_col = ((-offset_x / cell_size).floor() as i32).max(0) as u32;
    let start_row = ((-offset_y / cell_size).floor() as i32).max(0) as u32;
    let end_col = (((width - offset_x) / cell_size).ceil() as u32).min(GRID_WIDTH);
    let end_row = (((height - offset_y) / cell_size).ceil() as u32).min(GRID_HEIGHT);

    // Draw visible checkboxes
    for row in start_row..end_row {
        for col in start_col..end_col {
            let bit_index = row * GRID_WIDTH + col;
            let is_checked = get_bit(&chunk_data, bit_index);

            let x = offset_x + (col as f64) * cell_size;
            let y = offset_y + (row as f64) * cell_size;

            ctx.set_fill_style_str(if is_checked {
                COLOR_CHECKED
            } else {
                COLOR_UNCHECKED
            });
            ctx.fill_rect(x + 0.5, y + 0.5, cell_size - 1.0, cell_size - 1.0);
        }
    }
}

fn handle_click(e: MouseEvent, state: &AppState, canvas_ref: &NodeRef<Canvas>) {
    let Some(canvas) = canvas_ref.get() else {
        return;
    };
    let rect = canvas.get_bounding_client_rect();
    let x = e.client_x() as f64 - rect.left();
    let y = e.client_y() as f64 - rect.top();

    let offset_x = state.offset_x.get();
    let offset_y = state.offset_y.get();
    let scale = state.scale.get();

    if let Some((col, row)) = canvas_to_grid(x, y, offset_x, offset_y, scale) {
        let bit_index = row * GRID_WIDTH + col;

        // Optimistic update (local only for now - SpacetimeDB integration in Chunk 4)
        let mut data = state.chunk_data.get();
        let current = get_bit(&data, bit_index);
        set_bit(&mut data, bit_index, !current);
        let new_count = count_bits(&data);
        state.chunk_data.set(data);
        state.checked_count.set(new_count);

        // Note: Server sync will be added in Chunk 4 (Task 18)
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

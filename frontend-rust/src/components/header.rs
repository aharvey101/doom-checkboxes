use crate::bookmark::generate_bookmark;
use crate::constants::CELL_SIZE;
use crate::doom;
use crate::state::AppState;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;

#[component]
pub fn Header(state: AppState) -> impl IntoView {
    let status_class = move || state.status.get().as_class();
    let status_text = move || state.status_message.get();

    let stats_text = move || {
        let scale = state.scale.get();
        format!("Zoom: {:.1}x | Shift+drag to pan, scroll to zoom", scale)
    };

    // Doom mode state
    let (doom_running, set_doom_running) = signal(false);

    // Go home button handler - reset to origin (0,0)
    let go_home = move |_| {
        state.offset_x.set(0.0);
        state.offset_y.set(0.0);
        state.scale.set(1.0);
    };

    // Eraser mode toggle
    let toggle_eraser = move |_| {
        state.eraser_mode.update(|mode| *mode = !*mode);
    };

    let eraser_class = move || {
        if state.eraser_mode.get() {
            "eraser-btn active"
        } else {
            "eraser-btn"
        }
    };

    // Brush size handler
    let on_size_change = move |ev: web_sys::Event| {
        let target = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok());
        if let Some(input) = target {
            if let Ok(size) = input.value().parse::<f64>() {
                state.brush_size.set(size);
            }
        }
    };

    let brush_size_label = move || {
        format!("Size: {:.0}px", state.brush_size.get())
    };

    // Go to Doom location
    let go_to_doom = move |_| {
        doom::go_to_doom_location(state);
    };

    // Toggle Doom mode
    let toggle_doom = move |_| {
        if doom_running.get() {
            doom::stop_doom_mode();
            set_doom_running.set(false);
            state.status_message.set("Doom stopped".to_string());
        } else {
            // Navigate to Doom location immediately so the user sees
            // the doom area (with a pre-populated empty chunk) right away,
            // before the Doom WASM binary finishes loading.
            doom::go_to_doom_location(state);

            // Start Doom asynchronously
            let state_clone = state;
            wasm_bindgen_futures::spawn_local(async move {
                match doom::start_doom_mode(state_clone).await {
                    Ok(()) => {
                        set_doom_running.set(true);
                        state_clone.status_message.set("Doom running!".to_string());
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("Failed to start Doom: {}", e).into());
                        state_clone.status_message.set(format!("Error: {}", e));
                    }
                }
            });
        }
    };

    // Copy link button handler
    let copy_link = move |_| {
        let offset_x = state.offset_x.get_untracked();
        let offset_y = state.offset_y.get_untracked();
        let scale = state.scale.get_untracked();

        // Get canvas size to find center position
        if let Some(window) = web_sys::window() {
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

            let cell_size = CELL_SIZE * scale;

            // Calculate center grid position
            let center_x = (canvas_w / 2.0 - offset_x) / cell_size;
            let center_y = (canvas_h / 2.0 - offset_y) / cell_size;

            // Generate bookmark URL
            let bookmark = generate_bookmark(center_x, center_y, scale);

            // Get current URL and append bookmark
            if let Ok(location) = window.location().href() {
                // Remove existing query string
                let base_url = location.split('?').next().unwrap_or(&location);
                let full_url = format!("{}?{}", base_url, bookmark);

                // Copy to clipboard using execCommand fallback (works in more browsers)
                copy_to_clipboard(&full_url);
                web_sys::console::log_1(&format!("Copied link: {}", full_url).into());

                // Show feedback
                state.status_message.set("Link copied!".to_string());

                // Reset status after 2 seconds
                let state_clone = state;
                let closure = Closure::once(Box::new(move || {
                    if state_clone.status_message.get_untracked() == "Link copied!" {
                        state_clone.status_message.set("Connected".to_string());
                    }
                }) as Box<dyn FnOnce()>);

                let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                    closure.as_ref().unchecked_ref(),
                    2000,
                );
                closure.forget();
            }
        }
    };

    // Doom button text
    let doom_btn_text = move || {
        if doom_running.get() {
            "Stop Doom"
        } else {
            "Play Doom"
        }
    };

    // Focus Doom controls (call JS function)
    let focus_doom = move |_| {
        if doom_running.get() {
            doom::toggle_doom_controls();
        }
    };

    view! {
        <div class="header">
            <h1>"Infinite Checkboxes"</h1>
            <div class=status_class>{status_text}</div>
            <div class="stats">{stats_text}</div>
            <button class="home-btn" on:click=go_home>"Go Home"</button>
            <button class=eraser_class on:click=toggle_eraser>"Eraser"</button>
            <div class="brush-size-control">
                <label class="size-label">{brush_size_label}</label>
                <input
                    type="range"
                    min="1"
                    max="50"
                    step="1"
                    prop:value=move || state.brush_size.get()
                    on:input=on_size_change
                    class="size-slider"
                />
            </div>
            <button class="doom-location-btn" on:click=go_to_doom>"Go to Doom"</button>
            <button class="doom-btn" on:click=toggle_doom>{doom_btn_text}</button>
            {move || doom_running.get().then(|| view! {
                <button class="doom-focus-btn" on:click=focus_doom>"Focus Doom"</button>
            })}
            <button class="copy-link-btn" on:click=copy_link>"Copy Link"</button>
        </div>
    }
}

// JavaScript function for clipboard copy (execCommand fallback for broad browser support)
#[wasm_bindgen(inline_js = "
export function copy_text_to_clipboard(text) {
    const ta = document.createElement('textarea');
    ta.value = text;
    ta.style.position = 'fixed';
    ta.style.left = '-9999px';
    document.body.appendChild(ta);
    ta.select();
    document.execCommand('copy');
    document.body.removeChild(ta);
}
")]
extern "C" {
    fn copy_text_to_clipboard(text: &str);
}

/// Copy text to clipboard using JavaScript interop
fn copy_to_clipboard(text: &str) {
    copy_text_to_clipboard(text);
}

use crate::bookmark::generate_bookmark;
use crate::constants::CELL_SIZE;
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

    view! {
        <div class="header">
            <h1>"1 Billion Checkboxes"</h1>
            <div class=status_class>{status_text}</div>
            <div class="stats">{stats_text}</div>
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

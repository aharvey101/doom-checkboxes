use leptos::prelude::*;

use crate::bookmark::parse_bookmark;
use crate::components::{CheckboxCanvas, Header};
use crate::constants::CELL_SIZE;
use crate::db::init_connection;
use crate::state::AppState;

const STYLES: &str = include_str!("styles.css");

#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();

    // Parse bookmark URL on mount and set initial viewport
    Effect::new(move || {
        if let Some(window) = web_sys::window() {
            if let Ok(search) = window.location().search() {
                let bookmark = parse_bookmark(&search);

                // If we have coordinates, calculate offset to center that position
                if bookmark.x != 0.0 || bookmark.y != 0.0 {
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
                }
            }
        }

        // Initialize SpacetimeDB connection
        init_connection(state);
    });

    view! {
        <style>{STYLES}</style>
        <Header state=state />
        <CheckboxCanvas state=state />
    }
}

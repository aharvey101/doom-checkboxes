use crate::constants::TOTAL_CHECKBOXES;
use crate::state::AppState;
use leptos::prelude::*;

#[component]
pub fn Header(state: AppState) -> impl IntoView {
    let status_class = move || state.status.get().as_class();
    let status_text = move || state.status_message.get();

    let stats_text = move || {
        let checked = state.checked_count.get();
        let scale = state.scale.get();
        format!(
            "{} / {} checked | Zoom: {:.1}x | Shift+drag to pan, scroll to zoom",
            format_number(checked as u64),
            format_number(TOTAL_CHECKBOXES),
            scale
        )
    };

    view! {
        <div class="header">
            <h1>"1 Million Checkboxes"</h1>
            <div class=status_class>{status_text}</div>
            <div class="stats">{stats_text}</div>
        </div>
    }
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, c);
    }
    result
}

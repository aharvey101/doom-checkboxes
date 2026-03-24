use crate::doom;
use crate::state::AppState;
use leptos::prelude::*;

#[component]
pub fn Header(state: AppState) -> impl IntoView {
    let status_class = move || state.status.get().as_class();
    let status_text = move || state.status_message.get();

    let (doom_running, set_doom_running) = signal(false);

    let toggle_doom = move |_| {
        if doom_running.get() {
            doom::stop_doom_mode();
            set_doom_running.set(false);
            state.status_message.set("Doom stopped".to_string());
        } else {
            let state_clone = state;
            wasm_bindgen_futures::spawn_local(async move {
                match doom::start_doom_mode(state_clone).await {
                    Ok(()) => {
                        set_doom_running.set(true);
                        state_clone.status_message.set("Doom running!".to_string());
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("Doom error: {}", e).into());
                        state_clone.status_message.set(format!("Error: {}", e));
                    }
                }
            });
        }
    };

    let doom_btn_text = move || if doom_running.get() { "Stop Doom" } else { "Play Doom" };

    let focus_doom = move |_| {
        if doom_running.get() { doom::toggle_doom_controls(); }
    };

    view! {
        <div class="header">
            <h1>"Doom Checkboxes"</h1>
            <div class=status_class>{status_text}</div>
            <button class="doom-btn" on:click=toggle_doom>{doom_btn_text}</button>
            {move || doom_running.get().then(|| view! {
                <button class="doom-focus-btn" on:click=focus_doom>"Focus Doom"</button>
            })}
        </div>
    }
}

use leptos::prelude::*;

use crate::components::{CheckboxCanvas, Header};
use crate::state::AppState;

const STYLES: &str = include_str!("styles.css");

#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();

    view! {
        <style>{STYLES}</style>
        <Header state=state />
        <CheckboxCanvas state=state />
    }
}

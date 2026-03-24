use leptos::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Error,
}

impl ConnectionStatus {
    pub fn as_class(&self) -> &'static str {
        match self {
            ConnectionStatus::Connecting => "status connecting",
            ConnectionStatus::Connected => "status connected",
            ConnectionStatus::Error => "status error",
        }
    }
}

#[derive(Clone, Copy)]
pub struct AppState {
    pub status: RwSignal<ConnectionStatus>,
    pub status_message: RwSignal<String>,
    pub render_version: RwSignal<u32>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            status: RwSignal::new(ConnectionStatus::Connecting),
            status_message: RwSignal::new("Connecting...".to_string()),
            render_version: RwSignal::new(0),
        }
    }
}

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
    // Connection status
    pub status: RwSignal<ConnectionStatus>,
    pub status_message: RwSignal<String>,

    // Checkbox data: 125KB for 1M bits
    pub chunk_data: RwSignal<Vec<u8>>,

    // Derived count
    pub checked_count: RwSignal<u32>,

    // Viewport state
    pub offset_x: RwSignal<f64>,
    pub offset_y: RwSignal<f64>,
    pub scale: RwSignal<f64>,

    // Drag state
    pub is_dragging: RwSignal<bool>,
    pub last_mouse_x: RwSignal<f64>,
    pub last_mouse_y: RwSignal<f64>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            status: RwSignal::new(ConnectionStatus::Connecting),
            status_message: RwSignal::new("Connecting...".to_string()),
            chunk_data: RwSignal::new(vec![0u8; 125_000]),
            checked_count: RwSignal::new(0),
            offset_x: RwSignal::new(0.0),
            offset_y: RwSignal::new(0.0),
            scale: RwSignal::new(1.0),
            is_dragging: RwSignal::new(false),
            last_mouse_x: RwSignal::new(0.0),
            last_mouse_y: RwSignal::new(0.0),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

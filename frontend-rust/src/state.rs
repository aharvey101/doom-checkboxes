use leptos::prelude::*;
use std::collections::{HashMap, HashSet};

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

    // Multi-chunk data storage
    pub loaded_chunks: RwSignal<HashMap<u32, Vec<u8>>>, // chunk_id -> data
    pub loading_chunks: RwSignal<HashSet<u32>>,         // chunks being fetched
    pub subscribed_chunks: RwSignal<HashSet<u32>>,      // active subscriptions

    // Viewport state
    pub offset_x: RwSignal<f64>,
    pub offset_y: RwSignal<f64>,
    pub scale: RwSignal<f64>,

    // Drag state (for panning with shift+drag)
    pub is_dragging: RwSignal<bool>,
    pub last_mouse_x: RwSignal<f64>,
    pub last_mouse_y: RwSignal<f64>,

    // Drawing state (for drag-to-fill)
    pub is_drawing: RwSignal<bool>,

    // Render throttling
    pub render_pending: RwSignal<bool>,

    // Skip next full render (used after immediate cell render)
    pub skip_next_render: RwSignal<bool>,

    // Trigger full render (incremented when server sends updates)
    pub render_version: RwSignal<u32>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            status: RwSignal::new(ConnectionStatus::Connecting),
            status_message: RwSignal::new("Connecting...".to_string()),
            loaded_chunks: RwSignal::new(HashMap::new()),
            loading_chunks: RwSignal::new(HashSet::new()),
            subscribed_chunks: RwSignal::new(HashSet::new()),
            offset_x: RwSignal::new(0.0),
            offset_y: RwSignal::new(0.0),
            scale: RwSignal::new(1.0),
            is_dragging: RwSignal::new(false),
            last_mouse_x: RwSignal::new(0.0),
            last_mouse_y: RwSignal::new(0.0),
            is_drawing: RwSignal::new(false),
            render_pending: RwSignal::new(false),
            skip_next_render: RwSignal::new(false),
            render_version: RwSignal::new(0),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

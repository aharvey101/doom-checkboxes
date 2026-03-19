use leptos::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::constants::USER_COLOR_KEY;

/// Pending update for batch sending: (chunk_id: i64, cell_offset: u32, r, g, b, checked)
pub type PendingUpdate = (i64, u32, u8, u8, u8, bool);

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

    // Multi-chunk data storage (keyed by chunk_id: i64)
    pub loaded_chunks: RwSignal<HashMap<i64, Vec<u8>>>, // chunk_id -> data
    pub loading_chunks: RwSignal<HashSet<i64>>,         // chunks being fetched
    pub subscribed_chunks: RwSignal<HashSet<i64>>,      // active subscriptions

    // Viewport state
    pub offset_x: RwSignal<f64>,
    pub offset_y: RwSignal<f64>,
    pub scale: RwSignal<f64>,

    // Drag state (for panning with shift+drag)
    pub is_dragging: RwSignal<bool>,
    pub last_mouse_x: RwSignal<f64>,
    pub last_mouse_y: RwSignal<f64>,

    // Drawing state (for drag-to-fill) - signed coords for infinite grid
    pub is_drawing: RwSignal<bool>,
    pub last_draw_col: RwSignal<Option<i32>>,
    pub last_draw_row: RwSignal<Option<i32>>,

    // Pending updates for batching (chunk_id, cell_offset, r, g, b, checked)
    pub pending_updates: RwSignal<Vec<PendingUpdate>>,

    // Render throttling
    pub render_pending: RwSignal<bool>,

    // Skip next full render (used after immediate cell render)
    pub skip_next_render: RwSignal<bool>,

    // Trigger full render (incremented when server sends updates)
    pub render_version: RwSignal<u32>,

    // User's color (RGB)
    pub user_color: RwSignal<(u8, u8, u8)>,

    // Touch state for mobile
    pub touch_count: RwSignal<u32>,         // Number of active touches
    pub last_touch_distance: RwSignal<f64>, // For pinch zoom
    pub last_touch_midpoint: RwSignal<(f64, f64)>, // For two-finger pan
    pub is_pinching: RwSignal<bool>,        // Two-finger gesture active
}

impl AppState {
    pub fn new() -> Self {
        let user_color = load_or_generate_user_color();

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
            last_draw_col: RwSignal::new(None),
            last_draw_row: RwSignal::new(None),
            pending_updates: RwSignal::new(Vec::new()),
            render_pending: RwSignal::new(false),
            skip_next_render: RwSignal::new(false),
            render_version: RwSignal::new(0),
            user_color: RwSignal::new(user_color),
            touch_count: RwSignal::new(0),
            last_touch_distance: RwSignal::new(0.0),
            last_touch_midpoint: RwSignal::new((0.0, 0.0)),
            is_pinching: RwSignal::new(false),
        }
    }
}

/// Load user color from localStorage, or generate and save a new one
fn load_or_generate_user_color() -> (u8, u8, u8) {
    let window = web_sys::window().expect("no window");
    let storage = window
        .local_storage()
        .expect("failed to get localStorage")
        .expect("localStorage not available");

    // Try to load existing color
    if let Ok(Some(color_str)) = storage.get_item(USER_COLOR_KEY) {
        if let Some(color) = parse_color(&color_str) {
            return color;
        }
    }

    // Generate random color
    let r = (js_sys::Math::random() * 256.0) as u8;
    let g = (js_sys::Math::random() * 256.0) as u8;
    let b = (js_sys::Math::random() * 256.0) as u8;

    // Save to localStorage
    let color_str = format!("{},{},{}", r, g, b);
    let _ = storage.set_item(USER_COLOR_KEY, &color_str);

    web_sys::console::log_1(&format!("Generated user color: rgb({}, {}, {})", r, g, b).into());

    (r, g, b)
}

/// Parse color string "r,g,b" into tuple
fn parse_color(s: &str) -> Option<(u8, u8, u8)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        return None;
    }
    let r = parts[0].parse().ok()?;
    let g = parts[1].parse().ok()?;
    let b = parts[2].parse().ok()?;
    Some((r, g, b))
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

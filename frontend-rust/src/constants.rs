// Cell size in pixels at 1x scale
pub const CELL_SIZE: f64 = 8.0;

// Chunk configuration - infinite grid with 1000x1000 chunks
pub const CHUNK_SIZE: u32 = 1_000; // 1000x1000 checkboxes per chunk
pub const CHECKBOXES_PER_CHUNK: usize = CHUNK_SIZE as usize * CHUNK_SIZE as usize; // 1,000,000

// 4 bytes per checkbox (R, G, B, A) for WebGL texture
pub const CHUNK_DATA_SIZE: usize = CHECKBOXES_PER_CHUNK * 4; // 4,000,000 bytes

// Zoom bounds
pub const MIN_SCALE: f64 = 0.1;
pub const MAX_SCALE: f64 = 10.0;

// Colors
pub const COLOR_UNCHECKED: &str = "#000000";
pub const COLOR_GRID: &str = "#000000";

// SpacetimeDB
pub const DATABASE_NAME: &str = "checkboxes";
pub const SPACETIMEDB_URI_LOCAL: &str = "ws://127.0.0.1:3000";
pub const SPACETIMEDB_URI_PROD: &str = "wss://maincloud.spacetimedb.com";

// localStorage keys
pub const USER_COLOR_KEY: &str = "checkbox_user_color";
pub const VIEWPORT_KEY: &str = "checkbox_viewport";

// Grid configuration: 40,000 x 25,000 = 1 billion checkboxes
pub const GRID_WIDTH: u32 = 40_000;
pub const GRID_HEIGHT: u32 = 25_000;
pub const TOTAL_CHECKBOXES: u64 = GRID_WIDTH as u64 * GRID_HEIGHT as u64;
pub const CELL_SIZE: f64 = 8.0;

// Chunk configuration
pub const CHUNK_SIZE: u32 = 1_000; // 1000x1000 checkboxes per chunk
pub const CHUNKS_X: u32 = 40; // GRID_WIDTH / CHUNK_SIZE
pub const CHUNKS_Y: u32 = 25; // GRID_HEIGHT / CHUNK_SIZE
pub const TOTAL_CHUNKS: u32 = CHUNKS_X * CHUNKS_Y; // 1000
pub const CHUNK_DATA_SIZE: usize = (CHUNK_SIZE as usize * CHUNK_SIZE as usize) / 8; // 125,000 bytes

// Zoom bounds
pub const MIN_SCALE: f64 = 0.1;
pub const MAX_SCALE: f64 = 10.0;

// Colors
pub const COLOR_CHECKED: &str = "#2ecc71";
pub const COLOR_UNCHECKED: &str = "#2c3e50";
pub const COLOR_GRID: &str = "#1a1a2e";

// SpacetimeDB
pub const DATABASE_NAME: &str = "checkboxes";
pub const SPACETIMEDB_URI_LOCAL: &str = "ws://127.0.0.1:3000";
pub const SPACETIMEDB_URI_PROD: &str = "wss://maincloud.spacetimedb.com";

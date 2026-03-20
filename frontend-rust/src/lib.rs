pub mod app;
pub mod bookmark;
pub mod components;
pub mod compression;
pub mod constants;
pub mod db;
pub mod doom;
pub mod state;
pub mod utils;
pub mod webgl;
pub mod worker;
pub mod worker_bridge;
pub mod ws_client;

// Alias for worker protocol to make it accessible as worker_protocol
pub use worker::protocol as worker_protocol;

// Re-export for convenience
pub use app::App;
// OLD EXPORTS - Worker handles networking now
// pub use db::{init_connection, toggle_checkbox};
pub use db::toggle_checkbox;
pub use state::{AppState, ConnectionStatus};
// OLD EXPORTS - Worker handles networking now
// pub use ws_client::{SharedClient, SpacetimeClient};

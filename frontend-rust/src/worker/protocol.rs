//! Message protocol for main thread <-> worker communication

use serde::{Deserialize, Serialize};

/// Messages sent from main thread to worker
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum MainToWorker {
    /// Initialize SpacetimeDB connection
    Connect {
        uri: String,
        database: String,
    },

    /// Subscribe to specific chunks
    Subscribe {
        chunk_ids: Vec<i64>,
    },

    /// Single checkbox update
    UpdateCheckbox {
        chunk_id: i64,
        cell_offset: u32,
        r: u8,
        g: u8,
        b: u8,
        checked: bool,
    },

    /// Batch checkbox updates (drag-to-fill, Doom frames)
    BatchUpdate {
        updates: Vec<(i64, u32, u8, u8, u8, bool)>,
    },

    /// Disconnect and clean up
    Disconnect,
}

/// Messages sent from worker to main thread
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum WorkerToMain {
    /// Chunk loaded from server (initial)
    ChunkInserted {
        chunk_id: i64,
        state: Vec<u8>,
        version: u64,
    },

    /// Chunk updated by another client
    ChunkUpdated {
        chunk_id: i64,
        state: Vec<u8>,
        version: u64,
    },

    /// Successfully connected
    Connected,

    /// Fatal error after retries exhausted
    FatalError {
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main_to_worker_serialization() {
        let msg = MainToWorker::UpdateCheckbox {
            chunk_id: 42,
            cell_offset: 100,
            r: 255,
            g: 0,
            b: 0,
            checked: true,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: MainToWorker = serde_json::from_str(&json).unwrap();

        match deserialized {
            MainToWorker::UpdateCheckbox {
                chunk_id,
                cell_offset,
                r,
                g,
                b,
                checked,
            } => {
                assert_eq!(chunk_id, 42);
                assert_eq!(cell_offset, 100);
                assert_eq!(r, 255);
                assert_eq!(g, 0);
                assert_eq!(b, 0);
                assert_eq!(checked, true);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_worker_to_main_serialization() {
        let msg = WorkerToMain::ChunkUpdated {
            chunk_id: 123,
            state: vec![1, 2, 3, 4],
            version: 5,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: WorkerToMain = serde_json::from_str(&json).unwrap();

        match deserialized {
            WorkerToMain::ChunkUpdated {
                chunk_id,
                state,
                version,
            } => {
                assert_eq!(chunk_id, 123);
                assert_eq!(state, vec![1, 2, 3, 4]);
                assert_eq!(version, 5);
            }
            _ => panic!("Wrong message type"),
        }
    }
}

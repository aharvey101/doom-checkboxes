// SpacetimeDB backend for collaborative checkboxes - v5.0 (delta-based sync)

use spacetimedb::{reducer, table, ReducerContext, SpacetimeType, Table};

/// Chunk size: 1 million checkboxes per chunk (1000x1000)
const CHECKBOXES_PER_CHUNK: usize = 1_000_000;

/// Legacy chunk data size (4 bytes per checkbox)
const CHUNK_DATA_SIZE: usize = 4_000_000;

// === Chunk Addressing ===

fn chunk_coords_to_id(x: i32, y: i32) -> i64 {
    ((x as i64) << 32) | ((y as u32) as i64)
}

fn chunk_id_to_coords(id: i64) -> (i32, i32) {
    let x = (id >> 32) as i32;
    let y = id as i32;
    (x, y)
}

// === Data Types ===

/// A single checkbox update for batch operations (with color)
#[derive(SpacetimeType)]
pub struct CheckboxUpdate {
    pub chunk_id: i64,
    pub cell_offset: u32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub checked: bool,
}

/// Stores full checkbox state per chunk (for initial load / persistence).
/// Subscribers use this for the initial snapshot only.
#[table(accessor = checkbox_chunk, public)]
pub struct CheckboxChunk {
    #[primary_key]
    pub chunk_id: i64,
    pub state: Vec<u8>, // 4MB blob for 1M checkboxes with colors
    pub version: u64,
}

/// Lightweight delta row for real-time sync.
/// Each row represents a single cell change. Subscribers get these as
/// small (~20 byte) inserts instead of full 4MB chunk retransmissions.
/// Rows are ephemeral — cleaned up periodically by the cleanup reducer.
#[table(accessor = checkbox_delta, public)]
pub struct CheckboxDelta {
    #[auto_inc]
    #[primary_key]
    pub seq: u64,
    pub chunk_id: i64,
    pub cell_offset: u32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub checked: bool,
}

/// Packed frame for real-time sync. One row = one batch of updates.
/// Contains pre-packed binary data: [N × 16 bytes per update]
/// Each update: [chunk_id: i64 LE] [cell_offset: u32 LE] [r] [g] [b] [checked]
/// SpacetimeDB sends one row insert instead of 50K individual delta rows.
#[table(accessor = checkbox_frame, public)]
pub struct CheckboxFrame {
    #[auto_inc]
    #[primary_key]
    pub seq: u64,
    pub data: Vec<u8>,  // packed updates, 16 bytes each
}

/// Set a checkbox with color at the given position
fn set_checkbox(data: &mut [u8], cell_index: usize, r: u8, g: u8, b: u8, checked: bool) {
    let byte_idx = cell_index * 4;
    if byte_idx + 3 < data.len() {
        data[byte_idx] = r;
        data[byte_idx + 1] = g;
        data[byte_idx + 2] = b;
        data[byte_idx + 3] = if checked { 0xFF } else { 0x00 };
    }
}

/// Update a single checkbox — write frame + delta for real-time sync.
#[reducer]
pub fn update_checkbox(
    ctx: &ReducerContext,
    chunk_id: i64,
    cell_offset: u32,
    r: u8,
    g: u8,
    b: u8,
    checked: bool,
) {
    // Pack single update as a frame
    let mut packed = Vec::with_capacity(16);
    packed.extend_from_slice(&chunk_id.to_le_bytes());
    packed.extend_from_slice(&cell_offset.to_le_bytes());
    packed.push(r);
    packed.push(g);
    packed.push(b);
    packed.push(if checked { 1 } else { 0 });

    ctx.db.checkbox_frame().insert(CheckboxFrame {
        seq: 0,
        data: packed,
    });

    ctx.db.checkbox_delta().insert(CheckboxDelta {
        seq: 0,
        chunk_id,
        cell_offset,
        r,
        g,
        b,
        checked,
    });
}

/// Batch update: pack into a single frame row for real-time sync.
/// Also writes individual deltas for snapshot_chunks to process.
#[reducer]
pub fn batch_update_checkboxes(ctx: &ReducerContext, updates: Vec<CheckboxUpdate>) {
    // Pack all updates into a single binary blob (16 bytes each)
    let mut packed = Vec::with_capacity(updates.len() * 16);
    for update in &updates {
        packed.extend_from_slice(&update.chunk_id.to_le_bytes());
        packed.extend_from_slice(&update.cell_offset.to_le_bytes());
        packed.push(update.r);
        packed.push(update.g);
        packed.push(update.b);
        packed.push(if update.checked { 1 } else { 0 });
    }

    // One row insert instead of 50K
    ctx.db.checkbox_frame().insert(CheckboxFrame {
        seq: 0, // auto_inc
        data: packed,
    });

    // Also write individual deltas for snapshot to process
    for update in updates {
        ctx.db.checkbox_delta().insert(CheckboxDelta {
            seq: 0,
            chunk_id: update.chunk_id,
            cell_offset: update.cell_offset,
            r: update.r,
            g: update.g,
            b: update.b,
            checked: update.checked,
        });
    }
}

/// Snapshot: apply all pending deltas to the full chunk state.
/// Only touches checkbox_chunk table — spectators unsubscribe from this,
/// so they won't receive the large TransactionUpdate.
#[reducer]
pub fn snapshot_chunks(ctx: &ReducerContext) {
    use std::collections::HashMap;

    let deltas: Vec<_> = ctx.db.checkbox_delta().iter().collect();
    if deltas.is_empty() {
        return;
    }

    // Group by chunk_id
    let mut chunk_updates: HashMap<i64, Vec<(u32, u8, u8, u8, bool)>> = HashMap::new();
    for d in &deltas {
        chunk_updates.entry(d.chunk_id).or_default().push((
            d.cell_offset, d.r, d.g, d.b, d.checked,
        ));
    }

    // Apply to chunk state
    for (chunk_id, cell_updates) in chunk_updates {
        if let Some(mut row) = ctx.db.checkbox_chunk().chunk_id().find(chunk_id) {
            for (cell_offset, r, g, b, checked) in cell_updates {
                set_checkbox(&mut row.state, cell_offset as usize, r, g, b, checked);
            }
            row.version += 1;
            ctx.db.checkbox_chunk().chunk_id().update(row);
        } else {
            let mut new_chunk = CheckboxChunk {
                chunk_id,
                state: vec![0u8; CHUNK_DATA_SIZE],
                version: 0,
            };
            for (cell_offset, r, g, b, checked) in cell_updates {
                set_checkbox(&mut new_chunk.state, cell_offset as usize, r, g, b, checked);
            }
            ctx.db.checkbox_chunk().insert(new_chunk);
        }
    }
}

/// Clean up old deltas and frames. Called separately from snapshot.
#[reducer]
pub fn cleanup_old_deltas(ctx: &ReducerContext, max_seq_to_delete: u64) {
    let old_delta_seqs: Vec<u64> = ctx
        .db
        .checkbox_delta()
        .iter()
        .filter(|d| d.seq <= max_seq_to_delete)
        .map(|d| d.seq)
        .collect();
    for seq in old_delta_seqs {
        ctx.db.checkbox_delta().seq().delete(seq);
    }

    let old_frame_seqs: Vec<u64> = ctx
        .db
        .checkbox_frame()
        .iter()
        .filter(|f| f.seq <= max_seq_to_delete)
        .map(|f| f.seq)
        .collect();
    for seq in old_frame_seqs {
        ctx.db.checkbox_frame().seq().delete(seq);
    }
}

/// Add a new chunk
#[reducer]
pub fn add_chunk(ctx: &ReducerContext, chunk_id: i64) {
    let new_chunk = CheckboxChunk {
        chunk_id,
        state: vec![0u8; CHUNK_DATA_SIZE],
        version: 0,
    };
    ctx.db.checkbox_chunk().insert(new_chunk);
}

/// Clean up old deltas to prevent the table from growing unbounded.
/// Call periodically (e.g., every 30 seconds) from a client or scheduled task.
/// Keeps only the most recent `keep_count` deltas.
#[reducer]
pub fn cleanup_deltas(ctx: &ReducerContext, keep_count: u64) {
    // Find the max seq
    let max_seq = ctx.db.checkbox_delta().iter().map(|d| d.seq).max().unwrap_or(0);

    if max_seq <= keep_count {
        return;
    }

    let cutoff = max_seq - keep_count;

    // Delete old deltas
    let old_seqs: Vec<u64> = ctx
        .db
        .checkbox_delta()
        .iter()
        .filter(|d| d.seq < cutoff)
        .map(|d| d.seq)
        .collect();

    for seq in old_seqs {
        ctx.db.checkbox_delta().seq().delete(seq);
    }
}

/// Clear all checkbox data (useful for testing)
#[reducer]
pub fn clear_all_checkboxes(ctx: &ReducerContext) {
    let chunk_ids: Vec<i64> = ctx
        .db
        .checkbox_chunk()
        .iter()
        .map(|row| row.chunk_id)
        .collect();

    for chunk_id in chunk_ids {
        ctx.db.checkbox_chunk().chunk_id().delete(chunk_id);
    }

    // Also clear deltas
    let delta_seqs: Vec<u64> = ctx
        .db
        .checkbox_delta()
        .iter()
        .map(|d| d.seq)
        .collect();

    for seq in delta_seqs {
        ctx.db.checkbox_delta().seq().delete(seq);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_checkbox() {
        let mut data = vec![0u8; 40];
        set_checkbox(&mut data, 0, 255, 0, 0, true);
        assert_eq!(data[0], 255);
        assert_eq!(data[3], 0xFF);

        set_checkbox(&mut data, 1, 0, 255, 0, true);
        assert_eq!(data[4], 0);
        assert_eq!(data[5], 255);
        assert_eq!(data[7], 0xFF);

        set_checkbox(&mut data, 0, 100, 100, 100, false);
        assert_eq!(data[0], 100);
        assert_eq!(data[3], 0x00);
    }

    #[test]
    fn test_chunk_coords_roundtrip() {
        let test_coords = [
            (0, 0), (1, 0), (0, 1), (1, 1),
            (-1, 0), (0, -1), (-1, -1),
            (1000, 2000), (-1000, -2000),
            (i32::MAX, i32::MAX), (i32::MIN, i32::MIN),
        ];

        for (x, y) in test_coords {
            let id = chunk_coords_to_id(x, y);
            let (rx, ry) = chunk_id_to_coords(id);
            assert_eq!((rx, ry), (x, y));
        }
    }
}

// SpacetimeDB backend for collaborative checkboxes - v2.0

use spacetimedb::{reducer, table, ReducerContext, SpacetimeType, Table};

/// A single checkbox update for batch operations
#[derive(SpacetimeType)]
pub struct CheckboxUpdate {
    pub chunk_id: u32,
    pub bit_offset: u32,
    pub checked: bool,
}

/// Stores checkbox state in chunks of 1 million checkboxes each  
/// Each chunk = 125KB (1,000,000 bits / 8 bytes per bit)
#[table(accessor = checkbox_chunk, public)]
pub struct CheckboxChunk {
    #[primary_key]
    pub chunk_id: u32,
    pub state: Vec<u8>, // 125KB blob for 1M checkboxes
    pub version: u64,   // For tracking updates
}

/// Set a bit at the given position in a byte vector
fn set_bit(data: &mut [u8], bit_index: usize, value: bool) {
    let byte_idx = bit_index / 8;
    let bit_idx = bit_index % 8;

    if byte_idx < data.len() {
        if value {
            data[byte_idx] |= 1 << bit_idx;
        } else {
            data[byte_idx] &= !(1 << bit_idx);
        }
    }
}

/// Update a single checkbox bit in a chunk
#[reducer]
pub fn update_checkbox(ctx: &ReducerContext, chunk_id: u32, bit_offset: u32, checked: bool) {
    // Try to find existing chunk by primary key
    if let Some(mut row) = ctx.db.checkbox_chunk().chunk_id().find(chunk_id) {
        set_bit(&mut row.state, bit_offset as usize, checked);
        row.version += 1;
        ctx.db.checkbox_chunk().chunk_id().update(row);
        return;
    }

    // If chunk doesn't exist, create it and set the bit
    let mut new_chunk = CheckboxChunk {
        chunk_id,
        state: vec![0u8; 125_000],
        version: 0,
    };
    set_bit(&mut new_chunk.state, bit_offset as usize, checked);
    ctx.db.checkbox_chunk().insert(new_chunk);
}

/// Batch update multiple checkboxes at once
/// Each update contains chunk_id, bit_offset, and checked state
#[reducer]
pub fn batch_update_checkboxes(ctx: &ReducerContext, updates: Vec<CheckboxUpdate>) {
    use std::collections::HashMap;

    // Group updates by chunk_id
    let mut chunk_updates: HashMap<u32, Vec<(u32, bool)>> = HashMap::new();

    for update in updates {
        chunk_updates
            .entry(update.chunk_id)
            .or_default()
            .push((update.bit_offset, update.checked));
    }

    // Apply all updates per chunk
    for (chunk_id, updates) in chunk_updates {
        if let Some(mut row) = ctx.db.checkbox_chunk().chunk_id().find(chunk_id) {
            for (bit_offset, checked) in updates {
                set_bit(&mut row.state, bit_offset as usize, checked);
            }
            row.version += 1;
            ctx.db.checkbox_chunk().chunk_id().update(row);
        } else {
            // Create new chunk
            let mut new_chunk = CheckboxChunk {
                chunk_id,
                state: vec![0u8; 125_000],
                version: 0,
            };
            for (bit_offset, checked) in updates {
                set_bit(&mut new_chunk.state, bit_offset as usize, checked);
            }
            ctx.db.checkbox_chunk().insert(new_chunk);
        }
    }
}

/// Add a new chunk for expanding to additional checkboxes
#[reducer]
pub fn add_chunk(ctx: &ReducerContext, chunk_id: u32) {
    let new_chunk = CheckboxChunk {
        chunk_id,
        state: vec![0u8; 125_000],
        version: 0,
    };
    ctx.db.checkbox_chunk().insert(new_chunk);
}

/// Clear all checkbox data (useful for testing)
#[reducer]
pub fn clear_all_checkboxes(ctx: &ReducerContext) {
    let chunk_ids: Vec<u32> = ctx
        .db
        .checkbox_chunk()
        .iter()
        .map(|row| row.chunk_id)
        .collect();

    for chunk_id in chunk_ids {
        ctx.db.checkbox_chunk().chunk_id().delete(chunk_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_bit() {
        let mut data = vec![0u8; 10];

        set_bit(&mut data, 0, true);
        assert_eq!(data[0] & 1, 1);

        set_bit(&mut data, 7, true);
        assert_eq!(data[0] & 0b10000000, 0b10000000);

        set_bit(&mut data, 8, true);
        assert_eq!(data[1] & 1, 1);

        set_bit(&mut data, 0, false);
        assert_eq!(data[0] & 1, 0);
    }

    #[test]
    fn test_chunk_size() {
        let chunk = CheckboxChunk {
            chunk_id: 0,
            state: vec![0u8; 125_000],
            version: 0,
        };
        assert_eq!(chunk.state.len() * 8, 1_000_000);
    }
}

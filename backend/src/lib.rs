// SpacetimeDB backend for collaborative checkboxes

use spacetimedb::{log, reducer, table, ReducerContext, Table};

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
pub fn update_checkbox(ctx: &ReducerContext, chunkId: u32, bitOffset: u32, checked: bool) {
    log::info!(
        "update_checkbox called: chunkId={}, bitOffset={}, checked={}",
        chunkId,
        bitOffset,
        checked
    );

    // Try to find existing chunk by primary key
    if let Some(mut row) = ctx.db.checkbox_chunk().chunk_id().find(chunkId) {
        log::info!("Found existing chunk with version: {}", row.version);

        // Always perform the bit operation - let SpacetimeDB detect if it's a real change
        set_bit(&mut row.state, bitOffset as usize, checked);
        row.version += 1;

        let new_version = row.version;

        // Use direct update - let SpacetimeDB handle change detection
        ctx.db.checkbox_chunk().chunk_id().update(row);

        log::info!("Successfully updated chunk to version: {}", new_version);
        return;
    }

    log::info!("Creating new chunk");

    // If chunk doesn't exist, create it and set the bit
    let mut new_chunk = CheckboxChunk {
        chunk_id: chunkId,
        state: vec![0u8; 125_000],
        version: 0,
    };
    set_bit(&mut new_chunk.state, bitOffset as usize, checked);
    ctx.db.checkbox_chunk().insert(new_chunk);

    log::info!("Successfully created new chunk");
}

/// Add a new chunk for expanding to additional checkboxes
#[reducer]
pub fn add_chunk(ctx: &ReducerContext, chunk_id: u32) {
    // Initialize a new chunk with 125KB (1M bits) of zeros
    let new_chunk = CheckboxChunk {
        chunk_id,
        state: vec![0u8; 125_000], // 1,000,000 bits / 8
        version: 0,
    };
    ctx.db.checkbox_chunk().insert(new_chunk);
}

/// Clear all checkbox data for test state reset
#[reducer]
pub fn clear_all_checkboxes(ctx: &ReducerContext) {
    log::info!("clear_all_checkboxes called - clearing all checkbox data");

    // Delete all rows from the checkbox_chunk table
    for row in ctx.db.checkbox_chunk().iter() {
        ctx.db.checkbox_chunk().chunk_id().delete(row.chunk_id);
    }

    log::info!("Successfully cleared all checkbox data");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_bit() {
        let mut data = vec![0u8; 10];

        // Test setting and getting bits
        set_bit(&mut data, 0, true);
        assert_eq!(data[0] & 1, 1);

        set_bit(&mut data, 7, true);
        assert_eq!(data[0] & 0b10000000, 0b10000000);

        set_bit(&mut data, 8, true);
        assert_eq!(data[1] & 1, 1);

        // Test toggling
        set_bit(&mut data, 0, false);
        assert_eq!(data[0] & 1, 0);
    }
}

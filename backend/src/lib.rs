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
pub fn update_checkbox(ctx: &ReducerContext, chunk_id: u32, bit_offset: u32, checked: bool) {
    log::info!(
        "update_checkbox called: chunk_id={}, bit_offset={}, checked={}",
        chunk_id,
        bit_offset,
        checked
    );

    // Try to find existing chunk by primary key
    if let Some(mut row) = ctx.db.checkbox_chunk().chunk_id().find(chunk_id) {
        log::info!("Found existing chunk with version: {}", row.version);

        // Always perform the bit operation - let SpacetimeDB detect if it's a real change
        set_bit(&mut row.state, bit_offset as usize, checked);
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
        chunk_id,
        state: vec![0u8; 125_000],
        version: 0,
    };
    set_bit(&mut new_chunk.state, bit_offset as usize, checked);
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

    // Collect all chunk_ids first to avoid iterator invalidation
    let chunk_ids: Vec<u32> = ctx
        .db
        .checkbox_chunk()
        .iter()
        .map(|row| row.chunk_id)
        .collect();

    // Delete all rows by chunk_id
    for chunk_id in chunk_ids {
        ctx.db.checkbox_chunk().chunk_id().delete(chunk_id);
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

    #[test]
    fn test_checkbox_chunk_structure() {
        // Test that CheckboxChunk has the expected structure
        let chunk = CheckboxChunk {
            chunk_id: 1,
            state: vec![0u8; 125_000],
            version: 0,
        };

        assert_eq!(chunk.chunk_id, 1);
        assert_eq!(chunk.state.len(), 125_000);
        assert_eq!(chunk.version, 0);

        // Verify the state vector is large enough for 1M bits
        assert_eq!(chunk.state.len() * 8, 1_000_000);
    }

    #[test]
    fn test_set_bit_bounds() {
        let mut data = vec![0u8; 125_000];

        // Test setting bits at the boundaries
        set_bit(&mut data, 0, true);
        assert_eq!(data[0] & 1, 1);

        set_bit(&mut data, 999_999, true); // Last bit in 1M bit range
        let last_byte_idx = 999_999 / 8;
        let last_bit_idx = 999_999 % 8;
        assert_eq!(data[last_byte_idx] & (1 << last_bit_idx), 1 << last_bit_idx);
    }

    // Note: Testing reducers like clear_all_checkboxes requires SpacetimeDB runtime context
    // Integration tests for reducers should be done with SpacetimeDB test framework
    // or through the CLI testing approach used in reset-test-state.js
}

// SpacetimeDB backend for Doom Checkboxes
//
// Single-purpose: broadcast Doom frames from player to spectators.
// One table: `frame` holds packed pixel data that spectators subscribe to.

use spacetimedb::{reducer, table, ReducerContext, SpacetimeType, Table};

/// Doom frame dimensions
const DOOM_WIDTH: usize = 640;
const DOOM_HEIGHT: usize = 400;
const PIXEL_COUNT: usize = DOOM_WIDTH * DOOM_HEIGHT; // 256,000

/// Packed Doom frame for real-time broadcast.
/// Each frame is a delta: only changed pixels are included.
/// Format: N × 7 bytes per pixel [offset_hi, offset_lo, offset_lo2, r, g, b, checked]
/// Using 3 bytes for offset allows up to 16M pixels (we need 256K).
#[table(accessor = frame, public)]
pub struct Frame {
    #[auto_inc]
    #[primary_key]
    pub id: u64,
    pub data: Vec<u8>,
}

/// Full frame state for initial load (so spectators joining mid-game see current state).
/// Single row, updated on each frame.
#[table(accessor = snapshot, public)]
pub struct Snapshot {
    #[primary_key]
    pub id: u64,
    /// Full RGBA state: 640×400×4 = 1,024,000 bytes
    pub state: Vec<u8>,
}

/// A pixel update in a batch
#[derive(SpacetimeType)]
pub struct PixelUpdate {
    pub offset: u32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub checked: bool,
}

/// Send a batch of pixel updates (a Doom frame delta).
/// Updates the snapshot and inserts a frame row for live subscribers.
#[reducer]
pub fn send_frame(ctx: &ReducerContext, updates: Vec<PixelUpdate>) {
    // Update snapshot
    let mut state = if let Some(row) = ctx.db.snapshot().id().find(0) {
        row.state.clone()
    } else {
        vec![0u8; PIXEL_COUNT * 4]
    };

    // Build packed delta for broadcast
    let mut delta = Vec::with_capacity(updates.len() * 7);
    for u in &updates {
        let off = u.offset;
        delta.push((off >> 16) as u8);
        delta.push((off >> 8) as u8);
        delta.push(off as u8);
        delta.push(u.r);
        delta.push(u.g);
        delta.push(u.b);
        delta.push(if u.checked { 1 } else { 0 });

        // Apply to snapshot
        let idx = (off as usize) * 4;
        if idx + 3 < state.len() {
            state[idx] = u.r;
            state[idx + 1] = u.g;
            state[idx + 2] = u.b;
            state[idx + 3] = if u.checked { 0xFF } else { 0x00 };
        }
    }

    // Upsert snapshot
    if ctx.db.snapshot().id().find(0).is_some() {
        ctx.db.snapshot().id().update(Snapshot { id: 0, state });
    } else {
        ctx.db.snapshot().insert(Snapshot { id: 0, state });
    }

    // Broadcast delta frame
    if !delta.is_empty() {
        ctx.db.frame().insert(Frame { id: 0, data: delta });
    }
}

/// Clean up old frame rows to prevent unbounded growth.
#[reducer]
pub fn cleanup_frames(ctx: &ReducerContext, max_id: u64) {
    let old: Vec<u64> = ctx.db.frame().iter()
        .filter(|f| f.id <= max_id)
        .map(|f| f.id)
        .collect();
    for id in old {
        ctx.db.frame().id().delete(id);
    }
}

/// Clear all data
#[reducer]
pub fn clear_all(ctx: &ReducerContext) {
    let ids: Vec<u64> = ctx.db.frame().iter().map(|f| f.id).collect();
    for id in ids { ctx.db.frame().id().delete(id); }
    if ctx.db.snapshot().id().find(0).is_some() {
        ctx.db.snapshot().id().delete(0);
    }
}

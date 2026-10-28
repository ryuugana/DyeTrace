//! Turning legend boxes into the five sample offsets.
//!
//! Offsets are measured from the pointer, because that is where the cursor goes
//! when a match is clicked.  One of the five samples is always the pixel under
//! the pointer itself, so `(0,0)` is part of every table.

use crate::markers::sample_point;
use crate::MAX_TARGETS;

fn wrap254(v: i32) -> i32 {
    v.rem_euclid(254)
}

/// Merge an offset, treating anything within 3 px as the same slot.
fn push_unique(table: &mut Vec<(i32, i32)>, offset: (i32, i32)) {
    for existing in table.iter() {
        let dx = (wrap254(offset.0 - existing.0 + 127) - 127).abs();
        let dy = (wrap254(offset.1 - existing.1 + 127) - 127).abs();
        if dx <= 3 && dy <= 3 {
            return;
        }
    }
    if table.len() < MAX_TARGETS {
        table.push(offset);
    }
}

/// Fallback for a frame with no markers: DyeRite's square cell of `cell` px.
pub fn cell_offsets(cell: i32) -> Vec<(i32, i32)> {
    let lo = -(cell / 2) - (cell & 1);
    let hi = cell / 2;
    vec![(0, 0), (lo, lo), (hi, lo), (lo, hi), (hi, hi)]
}

/// Build the table from the box centres the client drew, relative to the
/// pointer position that produced them.
pub fn from_boxes(boxes: &[(i32, i32)], pointer: (i32, i32)) -> Vec<(i32, i32)> {
    let mut table = Vec::with_capacity(MAX_TARGETS);

    for centre in boxes {
        let (sx, sy) = sample_point(*centre);
        push_unique(&mut table, (sx - pointer.0, sy - pointer.1));
    }
    // The slot under the pointer has no box of its own when the pointer covers
    // it, and it is always sampled at the pointer.
    push_unique(&mut table, (0, 0));

    table
}

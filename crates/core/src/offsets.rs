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

/// The dye window numbers its five preview swatches, and the samples behind them
/// sit around the cursor: slot 1 is the pixel under the cursor, 2 and 3 are the
/// samples above it (left, then right), and 4 and 5 the ones below.
pub const SLOT_COUNT: usize = 5;
/// The slot that the pointer itself samples.
pub const SLOT_CENTRE: usize = 0;

/// The slot a sample belongs to, judged by where it sits around the cursor.
fn quadrant(dx: i32, dy: i32) -> usize {
    match (dy < 0, dx < 0) {
        (true, true) => 1,   // above, left
        (true, false) => 2,  // above, right
        (false, true) => 3,  // below, left
        (false, false) => 4, // below, right
    }
}

/// Pair every measured offset with the slot the dye window numbers it by.
///
/// A sample the detection missed keeps its neighbours' numbers, so a partial
/// read still means the same thing: if the top-left sample is not visible, the
/// top-right one is still slot 3.  Two samples cannot share a slot, so a second
/// one in the same corner takes the lowest free slot instead.
pub fn by_slot(table: &[(i32, i32)]) -> Vec<(usize, (i32, i32))> {
    let mut used = [false; SLOT_COUNT];
    let mut pairs: Vec<(usize, (i32, i32))> = Vec::with_capacity(table.len());

    let centre = table.iter().find(|(dx, dy)| *dx == 0 && *dy == 0);
    if let Some(offset) = centre {
        used[SLOT_CENTRE] = true;
        pairs.push((SLOT_CENTRE, *offset));
    }

    for offset in table {
        if Some(offset) == centre {
            continue;
        }
        let mut slot = quadrant(offset.0, offset.1);
        if used[slot] {
            match used.iter().position(|taken| !taken) {
                Some(free) => slot = free,
                None => continue,
            }
        }
        used[slot] = true;
        pairs.push((slot, *offset));
    }

    pairs.sort_by_key(|(slot, _)| *slot);
    pairs
}

/// The offsets in slot order, together with the slot of each entry.
pub fn ordered(table: &[(i32, i32)]) -> (Vec<(i32, i32)>, Vec<usize>) {
    let pairs = by_slot(table);
    (
        pairs.iter().map(|(_, offset)| *offset).collect(),
        pairs.iter().map(|(slot, _)| *slot).collect(),
    )
}

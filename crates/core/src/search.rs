//! Sampling with wrap-around, and the search over the palette.
//!
//! The dye window wraps a sample that falls outside the palette to the opposite
//! side, modulo 256; positions 254 and 255 are the separator and the window
//! background, so a sample landing there is unusable rather than a miss.

use crate::colour::matches;
use crate::image::{Image, Rgb};
use crate::{SCAN_H, SCAN_W};

/// A candidate position and how well it scored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub x: i32,
    pub y: i32,
    /// How many of the samples held their colour.
    pub count: usize,
    /// Summed squared channel error, used to break ties.
    pub distance: i64,
}

/// Wrap a coordinate the way the client does, modulo 256.
pub fn wrap256(v: i32) -> i32 {
    v.rem_euclid(256)
}

/// Colour at a wrapped position, or `None` if it lands on 254/255.
pub fn sample(grid: &Image, x: i32, y: i32) -> Option<Rgb> {
    let wx = wrap256(x);
    let wy = wrap256(y);
    if wx as usize >= SCAN_W || wy as usize >= SCAN_H {
        return None;
    }
    Some(grid.get(wx as usize, wy as usize))
}

/// Score one candidate position.
///
/// `colours` is per offset; `None` disables that slot, and slots beyond the
/// table length are ignored.  Unreadable samples are skipped rather than
/// penalised, which keeps "at least N" monotone: a sample we cannot read can
/// only add to the true count.
pub fn evaluate(
    grid: &Image,
    offsets: &[(i32, i32)],
    colours: &[Option<Rgb>],
    tolerance: u8,
    x: i32,
    y: i32,
) -> (usize, i64) {
    let mut count = 0;
    let mut distance = 0i64;

    for (i, offset) in offsets.iter().enumerate() {
        let want = match colours.get(i) {
            Some(Some(c)) => *c,
            _ => continue,
        };
        let px = match sample(grid, x + offset.0, y + offset.1) {
            Some(p) => p,
            None => continue,
        };
        let dr = px.r as i64 - want.r as i64;
        let dg = px.g as i64 - want.g as i64;
        let db = px.b as i64 - want.b as i64;
        distance += dr * dr + dg * dg + db * db;
        if matches(px, want, tolerance) {
            count += 1;
        }
    }

    (count, distance)
}

/// Search the whole palette for the position with the most matching samples.
/// Ties go to the smaller colour error.
pub fn search_with(
    grid: &Image,
    offsets: &[(i32, i32)],
    colours: &[Option<Rgb>],
    tolerance: u8,
) -> Option<Match> {
    let mut best: Option<Match> = None;

    for y in 0..SCAN_H as i32 {
        for x in 0..SCAN_W as i32 {
            let (count, distance) = evaluate(grid, offsets, colours, tolerance, x, y);
            let better = match best {
                None => true,
                Some(b) => count > b.count || (count == b.count && distance < b.distance),
            };
            if better {
                best = Some(Match {
                    x,
                    y,
                    count,
                    distance,
                });
            }
        }
    }

    best
}

/// Search where every slot wants the same colour.
pub fn search(grid: &Image, offsets: &[(i32, i32)], want: Rgb, tolerance: u8) -> Option<Match> {
    let colours = vec![Some(want); offsets.len()];
    search_with(grid, offsets, &colours, tolerance)
}

/// What a search should ask for: one colour per sample, and how many of them
/// must hold it.
///
/// Chosen samples take priority over the count - only they are asked for the
/// colour, and every one of them must match.  Without a choice every sample is
/// asked, and `slots` of them must match.  An index the table does not have is
/// ignored rather than treated as a miss.
pub fn plan(
    len: usize,
    want: Rgb,
    slots: usize,
    only: &[usize],
) -> (Vec<Option<Rgb>>, usize) {
    let chosen: Vec<usize> = only.iter().copied().filter(|index| *index < len).collect();
    if !chosen.is_empty() {
        let mut colours = vec![None; len];
        for index in &chosen {
            colours[*index] = Some(want);
        }
        return (colours, chosen.len());
    }
    let need = slots.clamp(1, len.max(1));
    (vec![Some(want); len], need)
}

/// The preview picture the original draws: white, with red where the colour is.
pub fn hit_map(grid: &Image, want: Rgb, tolerance: u8) -> Vec<u32> {
    let mut out = vec![crate::colour::COL_WHITE; SCAN_W * SCAN_H];
    for y in 0..SCAN_H {
        for x in 0..SCAN_W {
            if matches(grid.get(x, y), want, tolerance) {
                out[y * SCAN_W + x] = crate::colour::COL_RED;
            }
        }
    }
    out
}

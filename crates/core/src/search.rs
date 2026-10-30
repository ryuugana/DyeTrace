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

/// What a search has to satisfy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Need {
    /// The colour asked for at each sample; `None` leaves that sample out of
    /// the count.
    pub colours: Vec<Option<Rgb>>,
    /// How many of the asked samples must hold the colour.
    pub count: usize,
    /// Samples that must hold it whatever the count says.
    pub must: Vec<usize>,
}

impl Need {
    /// How many samples are asked for the colour.
    pub fn asked(&self) -> usize {
        self.colours.iter().filter(|colour| colour.is_some()).count()
    }
}

/// What a search should ask for, given the spots the player wants and the slots
/// they chose.
///
/// The spots number is the floor, and the chosen slots narrow where the hits may
/// come from:
///
/// * no slots chosen - `spots` of the five samples must hold the colour;
/// * fewer slots than spots - every chosen slot must hold it, and the rest of
///   the spots may come from any of the other samples;
/// * as many slots as spots - same thing, which leaves exactly the chosen slots
///   to fill the requirement;
/// * more slots than spots - only the chosen slots are asked, and any `spots` of
///   them will do.
///
/// A chosen slot the table does not have is ignored rather than treated as a
/// miss.
pub fn plan(len: usize, want: Rgb, spots: usize, chosen: &[usize]) -> Need {
    let picked: Vec<usize> = chosen
        .iter()
        .copied()
        .filter(|index| *index < len)
        .collect();
    let spots = spots.clamp(1, len.max(1));

    if picked.is_empty() {
        return Need {
            colours: vec![Some(want); len],
            count: spots,
            must: Vec::new(),
        };
    }

    if picked.len() > spots {
        let mut colours = vec![None; len];
        for index in &picked {
            colours[*index] = Some(want);
        }
        return Need {
            colours,
            count: spots,
            must: Vec::new(),
        };
    }

    Need {
        colours: vec![Some(want); len],
        count: spots,
        must: picked,
    }
}

/// Does the sample at `index` hold its colour at this position?
///
/// A sample the grid cannot answer for - the separator or the window background
/// - cannot be guaranteed, so it counts as a miss here.
fn holds(
    grid: &Image,
    offsets: &[(i32, i32)],
    need: &Need,
    tolerance: u8,
    x: i32,
    y: i32,
    index: usize,
) -> bool {
    let Some(want) = need.colours.get(index).copied().flatten() else {
        return false;
    };
    let Some(offset) = offsets.get(index) else {
        return false;
    };
    match sample(grid, x + offset.0, y + offset.1) {
        Some(px) => matches(px, want, tolerance),
        None => false,
    }
}

/// The best position a search found, and whether it meets the requirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Best {
    pub at: Match,
    pub met: bool,
}

fn beats(candidate: &Match, best: &Option<Match>) -> bool {
    match best {
        None => true,
        Some(b) => {
            candidate.count > b.count
                || (candidate.count == b.count && candidate.distance < b.distance)
        }
    }
}

/// Search for the best position, preferring one that satisfies `need`.
///
/// The returned position is the best that meets the requirement if there is
/// one, otherwise the best that came closest, so a failed attempt can say what
/// it found instead of only that it found nothing.
pub fn search_need(
    grid: &Image,
    offsets: &[(i32, i32)],
    need: &Need,
    tolerance: u8,
) -> Option<Best> {
    let mut met: Option<Match> = None;
    let mut any: Option<Match> = None;

    for y in 0..SCAN_H as i32 {
        for x in 0..SCAN_W as i32 {
            let (count, distance) = evaluate(grid, offsets, &need.colours, tolerance, x, y);
            let candidate = Match {
                x,
                y,
                count,
                distance,
            };
            let satisfies = count >= need.count
                && need
                    .must
                    .iter()
                    .all(|index| holds(grid, offsets, need, tolerance, x, y, *index));

            if satisfies && beats(&candidate, &met) {
                met = Some(candidate);
            }
            if beats(&candidate, &any) {
                any = Some(candidate);
            }
        }
    }

    match met {
        Some(at) => Some(Best { at, met: true }),
        None => any.map(|at| Best { at, met: false }),
    }
}

/// The preview picture: white, with red where the colour is.
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

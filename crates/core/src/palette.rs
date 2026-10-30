//! Locating the dye palette inside a window grab.
//!
//! A palette is a 256x256 square whose four borders are edges and whose
//! interior varies.  That property is independent of the colours it contains,
//! so the square detector works even when the palette is mostly black.

use crate::image::Image;
use crate::{CAP_H, CAP_W, SCAN_H, SCAN_W};

/// How far a sampled border may sit from the real edge.
pub const BORDER_SLACK: i32 = 2;

/// Coarsest score that counts as a real square.
const MIN_EDGE: i32 = 24;
const MIN_RANGE: i32 = 30;

/// How far apart the cheap pass looks, how many corners it shortlists for the
/// full test, and how often it samples a border while shortlisting.
const COARSE_STEP: i32 = 4;
const SHORTLIST: usize = 8;
const COARSE_SAMPLE: usize = 16;

/// The dye form draws a one-pixel black line around the gradient.  A pixel at or
/// below this luminance is that line, and this much of each side has to be it.
const FRAME_BLACK: i32 = 16;
const FRAME_SHARE: f32 = 0.95;

/// Percentage of the grab that looks like a colour gradient.
pub fn gradient_percent(capture: &Image) -> u32 {
    let mut colourful = 0u32;
    for y in 0..SCAN_H {
        for x in 0..SCAN_W {
            if capture.get(x, y).is_colourful() {
                colourful += 1;
            }
        }
    }
    colourful * 100 / (SCAN_W * SCAN_H) as u32
}

fn mean_gradient_h(img: &Image, x: i32, y: i32, len: i32) -> i32 {
    let mut sum = 0;
    for i in 0..len {
        let here = img.at(x + i, y).luminance();
        let above = img.at(x + i, y - 1).luminance();
        sum += (here - above).abs();
    }
    sum / len.max(1)
}

fn mean_gradient_v(img: &Image, x: i32, y: i32, len: i32) -> i32 {
    let mut sum = 0;
    for i in 0..len {
        let here = img.at(x, y + i).luminance();
        let left = img.at(x - 1, y + i).luminance();
        sum += (here - left).abs();
    }
    sum / len.max(1)
}

/// Score a candidate square: strongest gradient found on each border within
/// [`BORDER_SLACK`], plus the luminance range of the interior.  The border
/// term is weighted double so a busy interior cannot beat a real square.
///
/// Returns `(edge, range, total)`.
pub fn square_score(img: &Image, x: i32, y: i32) -> (i32, i32, i32) {
    let mut top = 0;
    let mut bottom = 0;
    let mut left = 0;
    let mut right = 0;

    for i in -BORDER_SLACK..=BORDER_SLACK {
        top = top.max(mean_gradient_h(img, x, y + i, CAP_W as i32));
        bottom = bottom.max(mean_gradient_h(
            img,
            x,
            y + CAP_H as i32 - 1 + i,
            CAP_W as i32,
        ));
        left = left.max(mean_gradient_v(img, x + i, y, CAP_H as i32));
        right = right.max(mean_gradient_v(
            img,
            x + CAP_W as i32 - 1 + i,
            y,
            CAP_H as i32,
        ));
    }

    let mut min = 255;
    let mut max = 0;
    let mut i = 0;
    while i < CAP_W as i32 {
        let mut j = 0;
        while j < CAP_H as i32 {
            let l = img.at(x + i, y + j).luminance();
            min = min.min(l);
            max = max.max(l);
            j += 8;
        }
        i += 8;
    }

    let edge = top + bottom + left + right;
    let range = max - min;
    (edge, range, edge * 2 + range)
}

/// Cheap per-side border strengths of a candidate square, sampled sparsely and
/// tried over the same slack the full score uses.  Returns
/// `(top, bottom, left, right)`.
///
/// The sides are kept apart on purpose: a corner with two real edges and two
/// blank ones - the flat strip of window above a palette, whose left and right
/// sides sit on the palette's own edges - must not score like a square.
fn coarse_sides(img: &Image, x: i32, y: i32) -> (i32, i32, i32, i32) {
    let mut top = 0;
    let mut bottom = 0;
    let mut left = 0;
    let mut right = 0;

    for d in -BORDER_SLACK..=BORDER_SLACK {
        let mut across_top = 0;
        let mut across_bottom = 0;
        for i in (0..CAP_W as i32).step_by(COARSE_SAMPLE) {
            let row = y + d;
            across_top +=
                (img.at(x + i, row).luminance() - img.at(x + i, row - 1).luminance()).abs();
            let row = y + CAP_H as i32 - 1 + d;
            across_bottom +=
                (img.at(x + i, row).luminance() - img.at(x + i, row - 1).luminance()).abs();
        }
        top = top.max(across_top);
        bottom = bottom.max(across_bottom);

        let mut down_left = 0;
        let mut down_right = 0;
        for j in (0..CAP_H as i32).step_by(COARSE_SAMPLE) {
            let col = x + d;
            down_left +=
                (img.at(col, y + j).luminance() - img.at(col - 1, y + j).luminance()).abs();
            let col = x + CAP_W as i32 - 1 + d;
            down_right +=
                (img.at(col, y + j).luminance() - img.at(col - 1, y + j).luminance()).abs();
        }
        left = left.max(down_left);
        right = right.max(down_right);
    }

    (top, bottom, left, right)
}

/// A cheap version of [`square_score`]: the borders and the interior are
/// sampled sparsely, and every side has to be present.  The weakest side is
/// weighted heavily, so a candidate with one blank border cannot rank at all.
fn coarse_score(img: &Image, x: i32, y: i32) -> i32 {
    let (top, bottom, left, right) = coarse_sides(img, x, y);
    let weakest = top.min(bottom).min(left).min(right);

    let mut min = 255;
    let mut max = 0;
    for j in (0..CAP_H as i32).step_by(COARSE_SAMPLE * 2) {
        for i in (0..CAP_W as i32).step_by(COARSE_SAMPLE * 2) {
            let level = img.at(x + i, y + j).luminance();
            min = min.min(level);
            max = max.max(level);
        }
    }

    weakest * 4 + (max - min)
}

/// Is the whole of a line black?  `None` if the line is not inside the frame.
fn black_line(img: &Image, start: (i32, i32), step: (i32, i32), len: i32) -> Option<bool> {
    let (x, y) = start;
    let (dx, dy) = step;
    let end = len - 1;
    let w = img.width() as i32;
    let h = img.height() as i32;
    if x < 0 || y < 0 || x + dx * end >= w || y + dy * end >= h {
        return None;
    }

    let mut black = 0;
    for i in 0..len {
        if img.at(x + dx * i, y + dy * i).luminance() <= FRAME_BLACK {
            black += 1;
        }
    }

    Some(black as f32 >= len as f32 * FRAME_SHARE)
}

/// Does the square at `origin` have the dye form's black line around it?
///
/// The gradient is 254 px inside a one-pixel black outline, and the grab starts
/// at the gradient's first pixel, so the outline is the 256 px square whose
/// top-left corner is one pixel above and left of `origin`.  Nothing else the
/// client draws looks like that - an inventory window has chrome for a border
/// with its icons well inside, a menu the same - so this is what tells the
/// palette apart from a panel that the square detector was fooled by.
pub fn has_frame(img: &Image, origin: (i32, i32)) -> bool {
    frame_corner(img, origin).is_some()
}

/// The palette's corner, if the black line is around `origin`.
///
/// The line sits one pixel outside the gradient, so it is looked for within a
/// pixel of `origin` in each direction: a corner the detector put a pixel off is
/// still the palette, and the corner the line implies is what comes back.
pub fn frame_corner(img: &Image, origin: (i32, i32)) -> Option<(i32, i32)> {
    for dy in -1..=1 {
        for dx in -1..=1 {
            let corner = (origin.0 + dx, origin.1 + dy);
            if ring_is_black(img, (corner.0 - 1, corner.1 - 1)) {
                return Some(corner);
            }
        }
    }
    None
}

/// Is the 256 px square starting at `ring` a black line right round?
///
/// A side that falls outside the grab cannot be judged and is not counted either
/// way, but at least two sides have to be.
fn ring_is_black(img: &Image, ring: (i32, i32)) -> bool {
    let (fx, fy) = ring;
    let side = (CAP_W - 1) as i32;

    let lines = [
        ((fx, fy), (1, 0)),        // above
        ((fx, fy + side), (1, 0)), // below
        ((fx, fy), (0, 1)),        // left
        ((fx + side, fy), (0, 1)), // right
    ];

    let mut counted = 0;
    let mut lined = 0;
    for (start, step) in lines {
        if let Some(black) = black_line(img, start, step, side + 1) {
            counted += 1;
            if black {
                lined += 1;
            }
        }
    }

    counted >= 2 && lined == counted
}

/// The corners worth scoring properly: each shortlisted position and every
/// neighbour within [`COARSE_STEP`] of it, in a stable order so ties settle the
/// same way on every run.
fn candidates(img: &Image, shortlist: &[(i32, i32, i32)]) -> Vec<(i32, i32)> {
    let w = img.width() as i32;
    let h = img.height() as i32;
    let mut spots: Vec<(i32, i32)> = Vec::new();

    for (_, cx, cy) in shortlist {
        for ry in cy - COARSE_STEP..=cy + COARSE_STEP {
            for rx in cx - COARSE_STEP..=cx + COARSE_STEP {
                if rx < 3
                    || ry < 3
                    || rx + CAP_W as i32 + BORDER_SLACK >= w
                    || ry + CAP_H as i32 + BORDER_SLACK >= h
                {
                    continue;
                }
                spots.push((rx, ry));
            }
        }
    }

    spots.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    spots.dedup();
    spots
}

/// Snap a corner to the exact border line; a 2 px misalignment would put frame
/// pixels inside the scanned area.
fn snap(img: &Image, x: i32, y: i32) -> (i32, i32) {
    let mut best_e = -1;
    let mut shift = 0;
    for d in -BORDER_SLACK..=BORDER_SLACK {
        let e = mean_gradient_h(img, x, y + d, CAP_W as i32);
        if e > best_e {
            best_e = e;
            shift = d;
        }
    }
    let y = y + shift;

    best_e = -1;
    shift = 0;
    for d in -BORDER_SLACK..=BORDER_SLACK {
        let e = mean_gradient_v(img, x + d, y, CAP_H as i32);
        if e > best_e {
            best_e = e;
            shift = d;
        }
    }
    (x + shift, y)
}

/// Find the palette by its square border.  Returns the palette's top-left
/// corner inside `img`.
///
/// Scoring every position of the window with the full border test costs roughly
/// a thousand million pixel reads - seconds of wait before anything can be
/// pressed.  A cheap pass therefore shortlists a handful of corners, and only
/// those get the full test.  Each of those is then snapped to its border and has
/// to show the black line the dye form draws around the gradient: the client's
/// own panels score well as squares, and acting on one of those means pressing
/// buttons that are not there.
pub fn find_square_field(img: &Image) -> Option<(i32, i32)> {
    let w = img.width() as i32;
    let h = img.height() as i32;

    let mut y = 3;
    let mut shortlist: Vec<(i32, i32, i32)> = Vec::with_capacity(SHORTLIST);
    while y + CAP_H as i32 + BORDER_SLACK < h {
        let mut x = 3;
        while x + CAP_W as i32 + BORDER_SLACK < w {
            let score = coarse_score(img, x, y);
            if shortlist.len() < SHORTLIST || score > shortlist[SHORTLIST - 1].0 {
                shortlist.push((score, x, y));
                shortlist.sort_by(|a, b| b.0.cmp(&a.0));
                shortlist.truncate(SHORTLIST);
            }
            x += COARSE_STEP;
        }
        y += COARSE_STEP;
    }

    // Score the shortlist properly, best first, and take the strongest corner
    // that is actually the palette.
    let mut scored: Vec<(i32, i32, i32, i32, i32)> = candidates(img, &shortlist)
        .into_iter()
        .map(|(rx, ry)| {
            let (edge, range, total) = square_score(img, rx, ry);
            (total, edge, range, rx, ry)
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));

    for (_, edge, range, rx, ry) in scored {
        if edge < MIN_EDGE || range < MIN_RANGE {
            continue;
        }

        let (bx, by) = snap(img, rx, ry);
        if let Some(corner) = frame_corner(img, (bx, by)) {
            return Some(corner);
        }
    }

    None
}

/// Fallback detector: the 254x254 window containing the most saturated pixels.
/// Used when the border test finds nothing.
pub fn find_field(img: &Image) -> Option<(i32, i32)> {
    let w = img.width();
    let h = img.height();
    if w < SCAN_W || h < SCAN_H {
        return None;
    }

    let mut best_score = -1i32;
    let (mut best_x, mut best_y) = (0, 0);

    let mut y = 0;
    while y + SCAN_H <= h {
        let mut x = 0;
        while x + SCAN_W <= w {
            let mut score = 0i32;
            let mut dy = 0;
            while dy < SCAN_H {
                let mut dx = 0;
                while dx < SCAN_W {
                    if img.get(x + dx, y + dy).is_colourful() {
                        score += 1;
                    }
                    dx += 4;
                }
                dy += 4;
            }
            if score > best_score {
                best_score = score;
                best_x = x;
                best_y = y;
            }
            x += 8;
        }
        y += 8;
    }

    // At least a quarter of the sampled points must be colourful.
    let sampled = (SCAN_W / 4) * (SCAN_H / 4);
    if best_score < (sampled / 4) as i32 {
        return None;
    }

    Some((best_x as i32, best_y as i32))
}

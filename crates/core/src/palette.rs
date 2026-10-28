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

/// Find the palette by its square border.  Returns the palette's top-left
/// corner inside `img`.
pub fn find_square_field(img: &Image) -> Option<(i32, i32)> {
    let w = img.width() as i32;
    let h = img.height() as i32;

    let mut best = -1;
    let (mut bx, mut by) = (0, 0);
    let (mut best_edge, mut best_range) = (0, 0);

    let mut y = 3;
    while y + CAP_H as i32 + BORDER_SLACK < h {
        let mut x = 3;
        while x + CAP_W as i32 + BORDER_SLACK < w {
            let (edge, range, total) = square_score(img, x, y);
            if total > best {
                best = total;
                bx = x;
                by = y;
                best_edge = edge;
                best_range = range;
            }
            x += 4;
        }
        y += 4;
    }

    if best < 0 {
        return None;
    }

    // A 4 px grid rarely lands on the border; refine around the best cell.
    for ry in by - 4..=by + 4 {
        for rx in bx - 4..=bx + 4 {
            if rx < 3
                || ry < 3
                || rx + CAP_W as i32 + BORDER_SLACK >= w
                || ry + CAP_H as i32 + BORDER_SLACK >= h
            {
                continue;
            }
            let (edge, range, total) = square_score(img, rx, ry);
            if total > best {
                best = total;
                bx = rx;
                by = ry;
                best_edge = edge;
                best_range = range;
            }
        }
    }

    if best_edge < MIN_EDGE || best_range < MIN_RANGE {
        return None;
    }

    // Snap to the exact border line; a 2 px misalignment would put frame
    // pixels inside the scanned area.
    let mut best_e = -1;
    let mut shift = 0;
    for d in -BORDER_SLACK..=BORDER_SLACK {
        let e = mean_gradient_h(img, bx, by + d, CAP_W as i32);
        if e > best_e {
            best_e = e;
            shift = d;
        }
    }
    by += shift;

    best_e = -1;
    shift = 0;
    for d in -BORDER_SLACK..=BORDER_SLACK {
        let e = mean_gradient_v(img, bx + d, by, CAP_H as i32);
        if e > best_e {
            best_e = e;
            shift = d;
        }
    }
    bx += shift;

    Some((bx, by))
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

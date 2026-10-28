//! Detecting the client's legend boxes.
//!
//! Each dye slot draws a crosshair (the sample point) joined by a short neck to
//! a box that displays the colour found there.  The box is a dark ring with a
//! flat interior.  Boxes are detected rather than crosshairs because the ring
//! is unambiguous against a random gradient.

use crate::image::{Image, Rgb};
use crate::MAX_TARGETS;

/// Ring colour: the outline is drawn in black.
fn ring(px: Rgb) -> bool {
    px.r < 70 && px.g < 70 && px.b < 70
}

/// Is the interior of the candidate box a single flat colour?
fn interior_is_flat(img: &Image, x: i32, y: i32, len: i32, hgt: i32) -> bool {
    let reference = img.at(x + 2, y + 2);
    let mut jy = y + 2;
    while jy <= y + hgt - 3 {
        let mut jx = x + 2;
        while jx <= x + len - 3 {
            let p = img.at(jx, jy);
            if (p.r as i32 - reference.r as i32).abs() > 8
                || (p.g as i32 - reference.g as i32).abs() > 8
                || (p.b as i32 - reference.b as i32).abs() > 8
            {
                return false;
            }
            jx += 1;
        }
        jy += 1;
    }
    true
}

/// Centres of the legend boxes, in raster order, at most `max` of them.
pub fn detect_boxes(img: &Image, max: usize) -> Vec<(i32, i32)> {
    let w = img.width() as i32;
    let h = img.height() as i32;
    let mut found: Vec<(i32, i32)> = Vec::with_capacity(MAX_TARGETS);

    'rows: for y in 0..h - 2 {
        for x in 0..w - 2 {
            if found.len() >= max {
                break 'rows;
            }
            if !ring(img.at(x, y)) {
                continue;
            }

            // horizontal run along the top edge
            let mut len = 0;
            while x + len < w && len <= 10 && ring(img.at(x + len, y)) {
                len += 1;
            }
            if !(7..=10).contains(&len) {
                continue;
            }

            // both vertical edges must have the same height
            let mut v_left = 0;
            while y + v_left < h && v_left <= 10 && ring(img.at(x, y + v_left)) {
                v_left += 1;
            }
            let mut v_right = 0;
            while y + v_right < h && v_right <= 10 && ring(img.at(x + len - 1, y + v_right)) {
                v_right += 1;
            }
            if (v_left - v_right).abs() > 1 || !(7..=10).contains(&v_left) {
                continue;
            }

            // bottom-right corner and bottom edge
            if !ring(img.at(x + len - 1, y + v_left - 1)) {
                continue;
            }
            let mut bottom = 0;
            for i in 0..len {
                if ring(img.at(x + i, y + v_left - 1)) {
                    bottom += 1;
                }
            }
            if bottom < len - 1 {
                continue;
            }

            // Nothing dark directly above or below the ring: rejects a hollow
            // inside a larger dark structure.  The leader line touches only the
            // left side, so it cannot break this.
            let mut blocked = false;
            for q in x + 2..=x + len - 3 {
                if ring(img.at(q, y - 1)) || ring(img.at(q, y + v_left)) {
                    blocked = true;
                    break;
                }
            }
            if blocked {
                continue;
            }

            if !interior_is_flat(img, x, y, len, v_left) {
                continue;
            }

            let centre = (x + len / 2, y + v_left / 2);
            let duplicate = found
                .iter()
                .any(|c| (c.0 - centre.0).abs() <= 3 && (c.1 - centre.1).abs() <= 3);
            if !duplicate {
                found.push(centre);
            }
        }
    }

    found
}

/// Colour displayed by a box, i.e. the colour sampled at its crosshair.
pub fn box_fill(img: &Image, centre: (i32, i32)) -> Rgb {
    img.at(centre.0, centre.1)
}

/// The sample point a box stands for.
pub fn sample_point(centre: (i32, i32)) -> (i32, i32) {
    (centre.0 + crate::CROSSHAIR_DX, centre.1)
}

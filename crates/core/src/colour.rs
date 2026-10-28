//! The colour test and the match mask.
//!
//! The original scans the palette once per colour and stores a mask byte, so a
//! pixel can be claimed by the "primary" colour (10) or by any other (1), and
//! a primary claim is never overwritten.

use crate::image::{Image, Rgb};
use crate::{SCAN_H, SCAN_W};

pub const MASK_NONE: u8 = 0;
pub const MASK_MATCH: u8 = 1;
pub const MASK_MATCH_ERR: u8 = 10;

pub const COL_WHITE: u32 = 0x00FF_FFFF;
pub const COL_RED: u32 = 0x00FF_0000;
pub const COL_GREEN: u32 = 0x0000_FF00;

/// Per-channel comparison with a tolerance; the same test the dye window uses.
pub fn matches(px: Rgb, want: Rgb, tolerance: u8) -> bool {
    let t = tolerance as i32;
    (px.r as i32 - want.r as i32).abs() <= t
        && (px.g as i32 - want.g as i32).abs() <= t
        && (px.b as i32 - want.b as i32).abs() <= t
}

/// The mask plus the picture shown in the preview pane.
#[derive(Clone)]
pub struct MatchMap {
    pub mask: Vec<u8>,
    pub preview: Vec<u32>,
    pub hits: usize,
}

impl Default for MatchMap {
    fn default() -> Self {
        Self::new()
    }
}

impl MatchMap {
    pub fn new() -> Self {
        Self {
            mask: vec![MASK_NONE; SCAN_W * SCAN_H],
            preview: vec![COL_WHITE; SCAN_W * SCAN_H],
            hits: 0,
        }
    }

    pub fn clear(&mut self) {
        self.mask.fill(MASK_NONE);
        self.preview.fill(COL_WHITE);
        self.hits = 0;
    }

    /// Mark every pixel of `capture` within `tolerance` of `want` and return how
    /// many pixels were newly marked.
    ///
    /// `detect_errors` selects the primary pass: it claims pixels as
    /// [`MASK_MATCH_ERR`] (red) and may upgrade a secondary claim.  The
    /// secondary pass only takes untouched pixels and paints them green.
    pub fn mark(&mut self, capture: &Image, want: Rgb, tolerance: u8, detect_errors: bool) -> usize {
        let mut marked = 0;

        for y in 0..SCAN_H {
            for x in 0..SCAN_W {
                if !matches(capture.get(x, y), want, tolerance) {
                    continue;
                }
                let idx = y * SCAN_W + x;
                if detect_errors && self.mask[idx] < MASK_MATCH_ERR {
                    self.mask[idx] = MASK_MATCH_ERR;
                    self.preview[idx] = COL_RED;
                    marked += 1;
                } else if !detect_errors && self.mask[idx] == MASK_NONE {
                    self.mask[idx] = MASK_MATCH;
                    self.preview[idx] = COL_GREEN;
                    marked += 1;
                }
            }
        }

        self.hits += marked;
        marked
    }
}

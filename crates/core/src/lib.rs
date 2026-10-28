//! Platform-independent matching core for DyeTrace.
//!
//! Nothing in this crate touches an operating system: every function takes
//! pixel buffers and plain numbers.  The platform layer feeds it captures and
//! the pointer position, and acts on the answer.
//!
//! Layout:
//!   image    - pixel buffer and colour type
//!   colour   - per-channel tolerance test and the match mask
//!   palette  - locating the dye palette in a window grab
//!   markers  - detecting the client's legend boxes
//!   offsets  - turning boxes into the five sample offsets
//!   search   - sampling with wrap-around and searching for a position
//!   selftest - the twelve behaviour checks

pub mod colour;
pub mod image;
pub mod markers;
pub mod offsets;
pub mod palette;
pub mod search;
pub mod selftest;

/// Size of the palette grab.
pub const CAP_W: usize = 256;
pub const CAP_H: usize = 256;

/// Scanned area inside the grab: the palette proper, excluding the separator
/// and background columns at 254 and 255.
pub const SCAN_W: usize = 254;
pub const SCAN_H: usize = 254;

/// Dye slots the client samples.
pub const MAX_TARGETS: usize = 5;

/// Fewest offsets that still make a usable table (the original assumes 4 or 5).
pub const MIN_SLOTS: usize = 4;

/// Distance from a legend box centre to the crosshair it belongs to.
pub const CROSSHAIR_DX: i32 = -10;

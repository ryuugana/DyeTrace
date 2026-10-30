//! The twelve behaviour checks: the matcher, the palette detector, the offsets
//! and the search, each pinned to what was measured on the client.

use crate::colour::{MatchMap, COL_RED};
use crate::image::{Image, Rgb};
use crate::markers;
use crate::offsets;
use crate::palette;
use crate::search;
use crate::{CAP_H, CAP_W, SCAN_W};

pub struct Check {
    pub name: &'static str,
    pub passed: bool,
}

/// A 256x256 grab with a colourful field, one exact patch and two lone pixels.
fn colourful_grab() -> Image {
    let mut img = Image::filled(CAP_W, CAP_H, Rgb::new(0x20, 0x20, 0x20));

    for y in 30..230usize {
        for x in 30..230usize {
            img.put(x, y, Rgb::new((x * 4) as u8, (y * 4) as u8, 0x40));
        }
    }
    for y in 100..140usize {
        for x in 100..140usize {
            img.put(x, y, Rgb::new(120, 60, 200));
        }
    }
    img.put(60, 60, Rgb::new(120, 60, 200));
    img.put(240, 240, Rgb::new(120, 60, 200));
    img
}

/// A 400x400 grab with a 254 px gradient on a flat background, inside the
/// 256 px black line the dye form draws around it.
fn square_on_background() -> Image {
    let mut img = Image::filled(400, 400, Rgb::new(0x30, 0x30, 0x30));
    for y in 40..40 + CAP_H as i32 - 2 {
        for x in 30..30 + CAP_W as i32 - 2 {
            img.put_i(x, y, Rgb::new((x * 3) as u8, (y * 3) as u8, 0x80));
        }
    }
    for i in -1..CAP_W as i32 - 1 {
        for (x, y) in [
            (30 + i, 39),
            (30 + i, 40 + CAP_H as i32 - 2),
            (29, 40 + i),
            (30 + CAP_W as i32 - 2, 40 + i),
        ] {
            img.put_i(x, y, Rgb::black());
        }
    }
    img
}

pub fn run() -> Vec<Check> {
    let mut checks = Vec::new();

    // 1. the matcher counts the patch plus the two lone pixels
    let img = colourful_grab();
    let target = Rgb::new(120, 60, 200);
    let mut map = MatchMap::new();
    let hits = map.mark(&img, target, 4, true);
    checks.push(Check {
        name: "matcher hit count = patch + 2 singles",
        passed: hits == 40 * 40 + 2,
    });

    // 2. a secondary pass must not recolour a primary hit
    map.mark(&img, target, 0, false);
    checks.push(Check {
        name: "primary hit stays red after 2nd pass",
        passed: map.preview[100 * SCAN_W + 100] == COL_RED,
    });

    // 3. tolerance is a real per-channel test
    let mut strict = MatchMap::new();
    let off_by_one = strict.mark(&img, Rgb::new(121, 60, 200), 0, true);
    checks.push(Check {
        name: "tolerance 0 rejects off-by-one colour",
        passed: off_by_one == 0,
    });

    // 4. and 5. the heuristic field detector finds the colourful area
    match palette::find_field(&img) {
        Some((fx, fy)) => {
            checks.push(Check {
                name: "colour field detected",
                passed: true,
            });
            checks.push(Check {
                name: "detected field near the colourful area",
                passed: (0..=60).contains(&fx) && (0..=60).contains(&fy),
            });
        }
        None => {
            checks.push(Check {
                name: "colour field detected",
                passed: false,
            });
            checks.push(Check {
                name: "detected field near the colourful area",
                passed: false,
            });
        }
    }

    // 6. and 7. a uniform image has neither a field nor hits
    let blank = Image::filled(CAP_W, CAP_H, Rgb::new(0xFF, 0xFF, 0xFF));
    checks.push(Check {
        name: "blank image rejected by field detector",
        passed: palette::find_field(&blank).is_none(),
    });
    let mut blank_map = MatchMap::new();
    checks.push(Check {
        name: "blank image has no hits",
        passed: blank_map.mark(&blank, target, 4, true) == 0,
    });

    // 8. black is a real dye colour and must be hittable
    let mut dark = Image::filled(CAP_W, CAP_H, Rgb::new(0x20, 0x40, 0x80));
    for y in 100..120usize {
        for x in 60..80usize {
            dark.put(x, y, Rgb::new(0, 0, 0));
        }
    }
    let mut dark_map = MatchMap::new();
    checks.push(Check {
        name: "black patch inside the palette is hittable",
        passed: dark_map.mark(&dark, Rgb::new(0, 0, 0), 0, true) == 20 * 20,
    });

    // 9. and 10. five distinct colours at a wrapping position are found again
    let cell = offsets::cell_offsets(21);
    let mut grid = Image::filled(CAP_W, CAP_H, Rgb::new(0x12, 0x34, 0x56));
    let anchor = (4i32, 250i32);
    let mut colours = Vec::with_capacity(cell.len());
    for (i, (dx, dy)) in cell.iter().enumerate() {
        let colour = Rgb::new(40 + i as u8 * 30, 80 + i as u8 * 20, 120 + i as u8 * 10);
        let x = search::wrap256(anchor.0 + dx) as usize;
        let y = search::wrap256(anchor.1 + dy) as usize;
        grid.put(x, y, colour);
        colours.push(Some(colour));
    }
    let found = search::search_with(&grid, &cell, &colours, 0);
    checks.push(Check {
        name: "picker finds all five samples",
        passed: found.map(|m| m.count) == Some(5),
    });
    checks.push(Check {
        name: "picker location correct with wrap-around",
        passed: found.map(|m| (m.x, m.y)) == Some(anchor),
    });

    // 11. and 12. the square detector finds the palette and snaps to its border
    let big = square_on_background();
    match palette::find_square_field(&big) {
        Some((fx, fy)) => {
            checks.push(Check {
                name: "square palette detector finds the field",
                passed: true,
            });
            checks.push(Check {
                name: "square detector snaps to the exact border",
                passed: (fx - 30).abs() <= 1 && (fy - 40).abs() <= 1,
            });
        }
        None => {
            checks.push(Check {
                name: "square palette detector finds the field",
                passed: false,
            });
            checks.push(Check {
                name: "square detector snaps to the exact border",
                passed: false,
            });
        }
    }

    // Keep the marker module referenced from the suite; box detection needs a
    // live frame, so it is exercised by the app's --replay path instead.
    let _ = markers::detect_boxes(&img, crate::MAX_TARGETS);

    checks
}

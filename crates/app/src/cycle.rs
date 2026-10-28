//! The auto cycle: read the legend, search, click.
//!
//! The picker is armed with the client's Pick Color button only when it is not
//! already armed, so a run never toggles a session it did not start.  The
//! searched image is always grabbed while the pointer is parked off the grid,
//! so it never contains the legend.
//!
//! Platform-agnostic: it only talks to the three traits, so the same code runs
//! against the Win32 backend and against the replay fakes.

use dyetrace_core::image::{Image, Rgb};
use dyetrace_core::{markers, offsets, palette, search, CAP_H, CAP_W, MAX_TARGETS, MIN_SLOTS};
use dyetrace_platform::traits::{GameWindow, Mouse, Rect, Screen, WindowId};

use std::thread::sleep;
use std::time::Duration;

/// How long to wait for the dye palette to appear.
const GRADIENT_WAIT: Duration = Duration::from_secs(4);
/// How long to wait for the client to redraw after an action.
const SETTLE: Duration = Duration::from_millis(320);
/// How many times to nudge the pointer while waiting for the legend.
const LEGEND_TRIES: usize = 4;
/// A grab below this percentage of saturated pixels is not a dye palette.
const MIN_GRADIENT: u32 = 25;

pub struct Options {
    pub colour: Rgb,
    pub variance: u8,
    /// How many of the five samples must hold the colour.
    pub slots: usize,
    /// Offsets of the client's Pick Color / Cancel buttons from the palette.
    pub pick_x: i32,
    pub cancel_x: i32,
    pub button_y: i32,
    pub max_cycles: usize,
    /// Search and report, but never click the gradient.
    pub dry_run: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            colour: Rgb::black(),
            variance: 0,
            slots: 1,
            pick_x: 147,
            cancel_x: 231,
            button_y: 397,
            max_cycles: 20,
            dry_run: false,
        }
    }
}

#[derive(Debug)]
pub enum Outcome {
    /// A match was found and (unless dry run) clicked.
    Matched {
        cycles: usize,
        x: i32,
        y: i32,
        count: usize,
        samples: usize,
        clicked: bool,
    },
    /// Every cycle cancelled: the colour never turned up.
    Exhausted { cycles: usize },
    /// The client or its palette never appeared.
    NoClient,
    /// The pointer could not be placed; nothing was clicked.
    PointerLost { cycles: usize },
}

/// Why the legend could not be read.
enum HoverFail {
    /// The pointer did not arrive where it was sent.
    Pointer,
    /// The pointer is on the grid but no boxes were drawn.
    NoLegend,
}

struct Board {
    rect: Rect,
    origin: (i32, i32),
}

/// Save a frame so a failed cycle can be inspected afterwards.
fn save_frame(path: &str, image: &Image) {
    let _ = dyetrace_platform::bmp::write_bmp(std::path::Path::new(path), image);
}

fn report(line: &str) {
    println!("{line}");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("scan.log")
    {
        use std::io::Write;
        let _ = writeln!(file, "{line}");
    }
}

/// Grab the palette rectangle, in screen coordinates.
fn grab(screen: &dyn Screen, board: &Board) -> Option<Image> {
    screen.capture(
        board.rect.x + board.origin.0,
        board.rect.y + board.origin.1,
        CAP_W as u32,
        CAP_H as u32,
    )
}

/// Wait until the palette is on screen and looks like a gradient.
fn locate(window: &dyn GameWindow, screen: &dyn Screen, id: WindowId) -> Option<Board> {
    let attempts = (GRADIENT_WAIT.as_millis() / 250).max(1) as usize;

    for _ in 0..attempts {
        if let Some(rect) = window.rect(id) {
            if let Some(frame) = screen.capture(rect.x, rect.y, rect.w as u32, rect.h as u32) {
                let origin =
                    palette::find_square_field(&frame).or_else(|| palette::find_field(&frame));
                if let Some(origin) = origin {
                    let board = Board { rect, origin };
                    if let Some(image) = grab(screen, &board) {
                        if palette::gradient_percent(&image) >= MIN_GRADIENT {
                            return Some(board);
                        }
                    }
                }
            }
        }
        sleep(Duration::from_millis(250));
    }

    None
}

fn button(board: &Board, offset_x: i32, offset_y: i32) -> (i32, i32) {
    (
        board.rect.x + board.origin.0 + offset_x,
        board.rect.y + board.origin.1 + offset_y,
    )
}

fn pointer_on_palette(mouse: &dyn Mouse, board: &Board) -> (i32, i32) {
    let (px, py) = mouse.position();
    (
        px - board.rect.x - board.origin.0,
        py - board.rect.y - board.origin.1,
    )
}

/// Move onto the grid and read the offsets from the client's legend.
///
/// The client only draws its squares while the picker is armed and only on real
/// pointer movement, so each attempt nudges the pointer.
fn read_offsets(
    mouse: &dyn Mouse,
    screen: &dyn Screen,
    board: &Board,
) -> Result<(Vec<(i32, i32)>, Image), HoverFail> {
    for attempt in 0..LEGEND_TRIES {
        if attempt == 0 {
            let (cx, cy) = button(board, CAP_W as i32 / 2, CAP_H as i32 / 2);
            if !mouse.move_to(cx, cy) {
                return Err(HoverFail::Pointer);
            }
        }

        // A move to the position it is already at produces no motion event.
        let step = 3 + attempt as i32;
        mouse.nudge(step, 2);
        sleep(SETTLE);
        mouse.nudge(-step, -2);
        sleep(Duration::from_millis(120));

        let pointer = pointer_on_palette(mouse, board);
        let live = match grab(screen, board) {
            Some(image) => image,
            None => return Err(HoverFail::Pointer),
        };

        let boxes = markers::detect_boxes(&live, MAX_TARGETS);
        report(&format!(
            "  legend {attempt}: boxes={} pointer=({},{})",
            boxes.len(),
            pointer.0,
            pointer.1
        ));

        if boxes.len() >= 2 {
            return Ok((offsets::from_boxes(&boxes, pointer), live));
        }
        save_frame("dt_live.bmp", &live);
    }

    Err(HoverFail::NoLegend)
}

pub fn run(window: &dyn GameWindow, screen: &dyn Screen, mouse: &dyn Mouse, opts: &Options) -> Outcome {
    let Some(id) = window.find() else {
        report("no client window (class \"Mabinogi\")");
        return Outcome::NoClient;
    };
    report(&format!("client found, focus={}", window.focus(id)));

    let Some(board) = locate(window, screen, id) else {
        report("no gradient - open the dye window and leave it visible");
        return Outcome::NoClient;
    };
    report(&format!(
        "palette ({},{}) in window at ({},{}), {}x{}",
        board.origin.0, board.origin.1, board.rect.x, board.rect.y, board.rect.w, board.rect.h
    ));

    let button_row = button(&board, opts.pick_x, opts.button_y);
    let mut cycles = 0usize;
    let mut armed_once = false;
    let mut arm_failures = 0usize;

    while cycles < opts.max_cycles {
        cycles += 1;
        window.focus(id);

        // Park off the grid so the grab has no legend in it.
        if !mouse.move_to(button_row.0, button_row.1) {
            report("pointer did not reach the button row");
            return Outcome::PointerLost { cycles };
        }
        sleep(Duration::from_millis(160));

        let mut grid = match grab(screen, &board) {
            Some(image) => image,
            None => {
                report("capture failed");
                return Outcome::PointerLost { cycles };
            }
        };

        let gradient = palette::gradient_percent(&grid);
        if gradient < MIN_GRADIENT {
            report(&format!(
                "cycle {cycles}: the dye window is gone (gradient {gradient}%) - stopping"
            ));
            return Outcome::NoClient;
        }

        // Read the offsets.  If the picker is not armed yet, arm it and re-grab,
        // because Pick Color rolls a new palette.
        let table = match read_offsets(mouse, screen, &board) {
            Ok((table, _)) => {
                armed_once = true;
                table
            }
            Err(HoverFail::Pointer) => {
                report("pointer did not reach the grid");
                return Outcome::PointerLost { cycles };
            }
            Err(HoverFail::NoLegend) => {
                report(&format!("cycle {cycles}: not armed - clicking Pick Color"));
                if !mouse.move_to(button_row.0, button_row.1) {
                    return Outcome::PointerLost { cycles };
                }
                mouse.click();
                sleep(Duration::from_millis(300));

                grid = match grab(screen, &board) {
                    Some(image) => image,
                    None => return Outcome::PointerLost { cycles },
                };
                let gradient = palette::gradient_percent(&grid);
                if gradient < MIN_GRADIENT {
                    report(&format!("  the dye window is gone (gradient {gradient}%) - stopping"));
                    return Outcome::NoClient;
                }

                match read_offsets(mouse, screen, &board) {
                    Ok((table, _)) => {
                        armed_once = true;
                        table
                    }
                    Err(HoverFail::Pointer) => return Outcome::PointerLost { cycles },
                    Err(HoverFail::NoLegend) => {
                        // Nothing has ever appeared: this is probably not the dye
                        // window, so stop instead of clicking around in the game.
                        arm_failures += 1;
                        save_frame("dt_grid.bmp", &grid);
                        if armed_once || arm_failures >= 2 {
                            report("  no legend after arming - the dye window does not seem to be open");
                            return Outcome::NoClient;
                        }
                        report("  still no legend - retrying");
                        continue;
                    }
                }
            }
        };

        report(&format!("cycle {cycles}: {} slots", table.len()));
        for (i, (dx, dy)) in table.iter().enumerate() {
            report(&format!("  slot {i} {dx:+},{dy:+}"));
        }

        if table.len() >= MIN_SLOTS {
            if let Some(best) = search::search(&grid, &table, opts.colour, opts.variance) {
                report(&format!(
                    "  best=({},{}) {}/{} need={} distance={}",
                    best.x,
                    best.y,
                    best.count,
                    table.len(),
                    opts.slots,
                    best.distance
                ));

                if best.count >= opts.slots {
                    let (mx, my) = button(&board, best.x, best.y);
                    if !mouse.move_to(mx, my) {
                        report("  pointer did not reach the match - not clicking");
                        return Outcome::PointerLost { cycles };
                    }
                    sleep(Duration::from_millis(120));

                    if !opts.dry_run {
                        mouse.click();
                    }
                    report(&format!(
                        "  {} ({mx},{my})",
                        if opts.dry_run { "would click" } else { "clicked" }
                    ));
                    return Outcome::Matched {
                        cycles,
                        x: mx,
                        y: my,
                        count: best.count,
                        samples: table.len(),
                        clicked: !opts.dry_run,
                    };
                }
            }
        }

        report("  not enough samples - cancelling");
        let (cx, cy) = button(&board, opts.cancel_x, opts.button_y);
        if mouse.move_to(cx, cy) {
            mouse.click();
        }
        sleep(SETTLE);
    }

    Outcome::Exhausted { cycles }
}

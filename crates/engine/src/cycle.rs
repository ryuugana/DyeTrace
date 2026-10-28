//! Preparing a search and acting on it.
//!
//! A dye attempt is: park the pointer off the grid, grab the gradient (so the
//! image has no legend in it), read the five sample offsets from the client's
//! legend, then search.  The picker is armed with the client's Pick Color button
//! only when the legend is not already showing, so a run never toggles a session
//! it did not start.
//!
//! Reading the client never raises it: the window is grabbed with
//! `PrintWindow` when the backend supports it, so the interface stays visible
//! and usable while a scan runs.

use dyetrace_core::image::{Image, Rgb};
use dyetrace_core::{markers, offsets, palette, search, CAP_H, CAP_W, MAX_TARGETS, MIN_SLOTS};
use dyetrace_platform::traits::{GameWindow, Mouse, Rect, Screen, WindowId};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
/// How many failed arming attempts before deciding the dye window is not open.
pub const ARM_TRIES: usize = 2;

#[derive(Clone, Debug)]
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
    /// Optional flag the caller can raise to abandon a long job.
    pub stop: Option<Arc<AtomicBool>>,
}

impl Options {
    /// Has the caller asked us to stop?
    pub fn halted(&self) -> bool {
        self.stop
            .as_ref()
            .map(|flag| flag.load(Ordering::Relaxed))
            .unwrap_or(false)
    }
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
            stop: None,
        }
    }
}

#[derive(Debug)]
pub enum Outcome {
    Matched {
        cycles: usize,
        x: i32,
        y: i32,
        count: usize,
        samples: usize,
        clicked: bool,
    },
    Exhausted {
        cycles: usize,
    },
    NoClient,
    PointerLost {
        cycles: usize,
    },
}

/// One prepared search: the grabbed gradient, the offset table and the result.
#[derive(Clone, Debug)]
pub struct Scan {
    /// The client's window rectangle.
    pub rect: Rect,
    /// The palette's top-left inside `rect`.
    pub origin: (i32, i32),
    /// The gradient as grabbed, without the legend.
    pub grid: Image,
    /// Where the pointer was when the offsets were read, in palette pixels.
    pub pointer: (i32, i32),
    /// Legend boxes found, in palette pixels.
    pub boxes: Vec<(i32, i32)>,
    /// Sample offsets relative to the pointer.
    pub table: Vec<(i32, i32)>,
    /// Best candidate and its score.
    pub best: Option<search::Match>,
    /// Whether this scan had to arm the picker itself.
    pub armed: bool,
}

impl Scan {
    /// Screen position of a palette pixel.
    pub fn to_screen(&self, x: i32, y: i32) -> (i32, i32) {
        (
            self.rect.x + self.origin.0 + x,
            self.rect.y + self.origin.1 + y,
        )
    }

    /// The preview the original draws: white with red where the colour is.
    pub fn hit_map(&self, colour: Rgb, variance: u8) -> Vec<u32> {
        search::hit_map(&self.grid, colour, variance)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanFail {
    /// No window of the client's class.
    NoClient,
    /// The dye window is not on screen, or its palette cannot be found.
    NoGradient,
    /// The pointer could not be placed.
    Pointer,
    /// The picker was armed twice and never drew its legend.
    NoLegend,
}

struct Board {
    rect: Rect,
    origin: (i32, i32),
}

struct Prepared {
    board: Board,
    grid: Image,
    pointer: (i32, i32),
    boxes: Vec<(i32, i32)>,
    table: Vec<(i32, i32)>,
    armed: bool,
}

/// The client's window rectangle together with a grab of its contents.
fn window_frame(
    window: &dyn GameWindow,
    screen: &dyn Screen,
    id: WindowId,
) -> Option<(Rect, Image)> {
    let rect = window.rect(id)?;
    if let Some(image) = screen.capture_window(id) {
        return Some((rect, image));
    }
    let image = screen.capture(rect.x, rect.y, rect.w as u32, rect.h as u32)?;
    Some((rect, image))
}

/// Cut the palette out of a window frame.
fn palette_grab(frame: &Image, origin: (i32, i32)) -> Option<Image> {
    frame.sub_image(origin.0, origin.1, CAP_W, CAP_H)
}

/// Wait until the palette is on screen and looks like a gradient.
pub fn locate(
    window: &dyn GameWindow,
    screen: &dyn Screen,
    id: WindowId,
) -> Option<(Rect, (i32, i32))> {
    let attempts = (GRADIENT_WAIT.as_millis() / 250).max(1) as usize;

    for _ in 0..attempts {
        if let Some((rect, frame)) = window_frame(window, screen, id) {
            let origin =
                palette::find_square_field(&frame).or_else(|| palette::find_field(&frame));
            if let Some(origin) = origin {
                if let Some(image) = palette_grab(&frame, origin) {
                    if palette::gradient_percent(&image) >= MIN_GRADIENT {
                        return Some((rect, origin));
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
fn read_offsets(
    window: &dyn GameWindow,
    screen: &dyn Screen,
    mouse: &dyn Mouse,
    id: WindowId,
    board: &Board,
) -> Result<(Vec<(i32, i32)>, (i32, i32), Vec<(i32, i32)>), ScanFail> {
    for attempt in 0..LEGEND_TRIES {
        if attempt == 0 {
            let (cx, cy) = button(board, CAP_W as i32 / 2, CAP_H as i32 / 2);
            if !mouse.move_to(cx, cy) {
                return Err(ScanFail::Pointer);
            }
        }

        // A move to the position it is already at produces no motion event, and
        // the client only redraws its legend on real movement.
        let step = 3 + attempt as i32;
        mouse.nudge(step, 2);
        sleep(SETTLE);
        mouse.nudge(-step, -2);
        sleep(Duration::from_millis(120));

        let pointer = pointer_on_palette(mouse, board);
        let (_, frame) = window_frame(window, screen, id).ok_or(ScanFail::Pointer)?;
        let live = palette_grab(&frame, board.origin).ok_or(ScanFail::Pointer)?;

        let boxes = markers::detect_boxes(&live, MAX_TARGETS);
        if boxes.len() >= 2 {
            let table = offsets::from_boxes(&boxes, pointer);
            return Ok((table, pointer, boxes));
        }
    }

    Err(ScanFail::NoLegend)
}

/// Park, grab, read the table — and arm the picker if it is not already armed.
fn prepare(
    window: &dyn GameWindow,
    screen: &dyn Screen,
    mouse: &dyn Mouse,
    id: WindowId,
    opts: &Options,
) -> Result<Prepared, ScanFail> {
    let Some((rect, origin)) = locate(window, screen, id) else {
        return Err(ScanFail::NoGradient);
    };
    let board = Board { rect, origin };
    let button_row = button(&board, opts.pick_x, opts.button_y);

    // Park off the grid so the grab has no legend in it.
    if !mouse.move_to(button_row.0, button_row.1) {
        return Err(ScanFail::Pointer);
    }
    sleep(Duration::from_millis(160));

    let mut grid = {
        let (_, frame) = window_frame(window, screen, id).ok_or(ScanFail::Pointer)?;
        palette_grab(&frame, origin).ok_or(ScanFail::NoGradient)?
    };
    if palette::gradient_percent(&grid) < MIN_GRADIENT {
        return Err(ScanFail::NoGradient);
    }

    let mut armed = false;

    let (table, pointer, boxes) = match read_offsets(window, screen, mouse, id, &board) {
        Ok(v) => v,
        Err(ScanFail::NoLegend) => {
            // Clicks land at screen coordinates: the client has to be in front
            // for them to reach it rather than whatever window covers it.
            window.focus(id);
            armed = true;
            let mut last = Err(ScanFail::NoLegend);
            for _ in 0..ARM_TRIES {
                if opts.halted() {
                    return Err(ScanFail::Pointer);
                }
                if !mouse.move_to(button_row.0, button_row.1) {
                    return Err(ScanFail::Pointer);
                }
                mouse.click();
                sleep(Duration::from_millis(300));

                let (_, frame) = window_frame(window, screen, id).ok_or(ScanFail::Pointer)?;
                grid = palette_grab(&frame, origin).ok_or(ScanFail::NoGradient)?;
                if palette::gradient_percent(&grid) < MIN_GRADIENT {
                    return Err(ScanFail::NoGradient);
                }

                last = read_offsets(window, screen, mouse, id, &board);
                if last.is_ok() {
                    break;
                }
            }
            last?
        }
        Err(other) => return Err(other),
    };

    Ok(Prepared {
        board,
        grid,
        pointer,
        boxes,
        table,
        armed,
    })
}

/// Prepare one search and report the best candidate, without clicking anything.
pub fn scan_once(
    window: &dyn GameWindow,
    screen: &dyn Screen,
    mouse: &dyn Mouse,
    opts: &Options,
) -> Result<Scan, ScanFail> {
    let id = window.find().ok_or(ScanFail::NoClient)?;
    let prep = prepare(window, screen, mouse, id, opts)?;
    let best = search::search(&prep.grid, &prep.table, opts.colour, opts.variance);

    Ok(Scan {
        rect: prep.board.rect,
        origin: prep.board.origin,
        grid: prep.grid,
        pointer: prep.pointer,
        boxes: prep.boxes,
        table: prep.table,
        best,
        armed: prep.armed,
    })
}

/// Run the full cycle until a match is clicked, the colour runs out, or the
/// dye window closes.
pub fn run(
    window: &dyn GameWindow,
    screen: &dyn Screen,
    mouse: &dyn Mouse,
    opts: &Options,
    mut report: impl FnMut(&str),
) -> Outcome {
    let Some(id) = window.find() else {
        report("no client window (class \"Mabinogi\")");
        return Outcome::NoClient;
    };

    if locate(window, screen, id).is_none() {
        report("no gradient - open the dye window and leave it visible");
        return Outcome::NoClient;
    }

    let mut cycles = 0usize;

    while cycles < opts.max_cycles {
        if opts.halted() {
            report("stopped");
            return Outcome::Exhausted { cycles };
        }
        cycles += 1;

        let prep = match prepare(window, screen, mouse, id, opts) {
            Ok(p) => p,
            Err(ScanFail::NoGradient) => {
                report(&format!("cycle {cycles}: the dye window is gone - stopping"));
                return Outcome::NoClient;
            }
            Err(ScanFail::NoClient) => return Outcome::NoClient,
            Err(ScanFail::Pointer) => {
                report("pointer could not be placed");
                return Outcome::PointerLost { cycles };
            }
            Err(ScanFail::NoLegend) => {
                report(&format!(
                    "cycle {cycles}: no legend even after arming - stopping"
                ));
                return Outcome::NoClient;
            }
        };

        report(&format!(
            "cycle {cycles}: {} slots (pointer {},{})",
            prep.table.len(),
            prep.pointer.0,
            prep.pointer.1
        ));

        if let Some(best) = search::search(&prep.grid, &prep.table, opts.colour, opts.variance) {
            report(&format!(
                "  best=({},{}) {}/{} need={}",
                best.x,
                best.y,
                best.count,
                prep.table.len(),
                opts.slots
            ));

            if best.count >= opts.slots && prep.table.len() >= MIN_SLOTS {
                window.focus(id);
                let (mx, my) = button(&prep.board, best.x, best.y);
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
                    samples: prep.table.len(),
                    clicked: !opts.dry_run,
                };
            }
        }

        report("  not enough samples - cancelling");
        window.focus(id);
        let (cx, cy) = button(&prep.board, opts.cancel_x, opts.button_y);
        if mouse.move_to(cx, cy) {
            mouse.click();
        }
        sleep(SETTLE);
    }

    Outcome::Exhausted { cycles }
}

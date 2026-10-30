//! Preparing a search and acting on it.
//!
//! A dye attempt is: park the pointer off the grid, grab the gradient (so the
//! image has no legend in it), read the five sample offsets from the client's
//! legend, then search.  Arming is the only part that presses buttons, and the
//! legend decides when: no legend means no colour has been chosen, so Pick Color
//! is safe to press; a live legend means the picker is already armed and the
//! button reads "Start Dyeing", where Cancel is safe and Pick Color is not.
//!
//! Reading the client never raises it: the window is grabbed with
//! `PrintWindow` when the backend supports it, so the interface stays visible
//! and usable while a scan runs.

use dyetrace_core::image::{Image, Rgb};
use dyetrace_core::{label, markers, offsets, palette, search, CAP_H, CAP_W, MAX_TARGETS, MIN_SLOTS};
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
/// How many Pick Color presses one attempt may make before giving up.
const MAX_PRESSES: usize = 2;
/// A grab below this percentage of saturated pixels is not a dye palette.
const MIN_GRADIENT: u32 = 25;

#[derive(Clone, Debug)]
pub struct Options {
    pub colour: Rgb,
    pub variance: u8,
    /// How many of the five samples must hold the colour.
    pub slots: usize,
    /// Preferred sample, as an index into the measured table.  When set it
    /// takes priority over `slots`: only that sample is asked for the colour.
    pub prefer: Option<usize>,
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
            prefer: None,
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
    /// The dye button's label could not be read, so nothing could be pressed.
    ButtonUnknown,
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

/// Grab the palette, retrying briefly.
///
/// A capture taken immediately after a click can still show the frame before the
/// redraw that the click caused - Pick Color rolls a new palette - so a single
/// grab is not enough to judge whether the palette is there.
fn settled_grab(
    window: &dyn GameWindow,
    screen: &dyn Screen,
    id: WindowId,
    origin: (i32, i32),
    log: &mut dyn FnMut(&str),
) -> Option<Image> {
    for attempt in 0..8 {
        if attempt > 0 {
            sleep(Duration::from_millis(180));
        }
        match window_frame(window, screen, id) {
            Some((_, frame)) => match palette_grab(&frame, origin) {
                Some(crop) => {
                    let pct = palette::gradient_percent(&crop);
                    log(&format!("    grab {attempt}: gradient {pct}%"));
                    if pct >= MIN_GRADIENT {
                        return Some(crop);
                    }
                }
                None => log(&format!(
                    "    grab {attempt}: palette rect outside the {}x{} frame",
                    frame.width(),
                    frame.height()
                )),
            },
            None => log(&format!("    grab {attempt}: window frame unavailable")),
        }
    }
    log("    the palette did not settle after the click");
    None
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

/// Where the pointer waits between clicks: clear of the grid and clear of both
/// buttons, but still inside the dye form - a pointer left in the game world is
/// taken over by the camera and wanders off on its own.
const PARK_DX: i32 = 128;
const PARK_DY: i32 = 300;

/// Park the pointer, with a small nudge so the motion is real.
///
/// The client only redraws its legend, its button label and its hover highlight
/// in response to pointer movement, so a pointer that is simply left in place
/// leaves the last state on screen.
fn park(mouse: &dyn Mouse, board: &Board) -> bool {
    let (x, y) = button(board, PARK_DX, PARK_DY);
    if !mouse.move_to(x, y) {
        return false;
    }
    sleep(Duration::from_millis(180));
    mouse.nudge(2, 1);
    sleep(Duration::from_millis(60));
    mouse.nudge(-2, -1);
    sleep(Duration::from_millis(160));
    true
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
    log: &mut dyn FnMut(&str),
) -> Result<(Vec<(i32, i32)>, (i32, i32), Vec<(i32, i32)>), ScanFail> {
    let mut best: Option<(Vec<(i32, i32)>, (i32, i32), Vec<(i32, i32)>)> = None;

    for attempt in 0..LEGEND_TRIES {
        if attempt == 0 {
            let (cx, cy) = button(board, CAP_W as i32 / 2, CAP_H as i32 / 2);
            log(&format!(
                "    move to ({cx},{cy}); rect ({},{}), origin ({},{})",
                board.rect.x, board.rect.y, board.origin.0, board.origin.1
            ));
            if !mouse.move_to(cx, cy) {
                return Err(ScanFail::Pointer);
            }
            log(&format!("    arrived at {:?}", mouse.position()));
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
        log(&format!(
            "    hover {attempt}: {} boxes, pointer ({},{})",
            boxes.len(),
            pointer.0,
            pointer.1
        ));
        if boxes.len() >= 2 {
            let table = offsets::from_boxes(&boxes, pointer);
            // The client draws all five samples, but one can be clipped by the
            // palette edge; a partial read is kept in case no better one comes,
            // and a full one ends the wait at once.
            if boxes.len() >= MIN_SLOTS {
                return Ok((table, pointer, boxes));
            }
            if best
                .as_ref()
                .map(|(_, _, b)| boxes.len() > b.len())
                .unwrap_or(true)
            {
                best = Some((table, pointer, boxes));
            }
        }
    }

    best.map_or(Err(ScanFail::NoLegend), Ok)
}

/// Park, grab, read the table — and arm the picker if it is not already armed.
///
/// The button's label decides what may be pressed, because the two faces are
/// not interchangeable. White "Pick Color" arms the picker. Grey "Start Dyeing"
/// means a colour has been chosen: pressing it applies the dye, and Cancel is
/// the only safe click. Cancel while the label still says "Pick Color" closes
/// the dye window - which is how an attempt was lost to a blind "reset" click.
///
/// So each press is gated on a fresh reading of the label, taken from the same
/// grab the gradient comes from:
///
/// * "Pick Color" - press it once, then grab the new roll;
/// * "Start Dyeing" - already armed, press nothing and grab what is there;
/// * unreadable - press nothing and stop.
///
/// A press that leaves no legend behind is retried once, and only after the
/// label has been read again and still says "Pick Color".
fn prepare(
    window: &dyn GameWindow,
    screen: &dyn Screen,
    mouse: &dyn Mouse,
    id: WindowId,
    opts: &Options,
    log: &mut dyn FnMut(&str),
) -> Result<Prepared, ScanFail> {
    // Raise the client before the pointer goes anywhere near it: a background
    // client draws no legend and receives no clicks, so the first attempt used
    // to fail before the switch happened.
    window.focus(id);

    let Some((rect, origin)) = locate(window, screen, id) else {
        return Err(ScanFail::NoGradient);
    };
    let board = Board { rect, origin };
    let button_row = button(&board, opts.pick_x, opts.button_y);

    // Park clear of both the grid and the buttons: the grab must not contain the
    // legend, and a hovered button draws a highlight that hides its label.
    if !park(mouse, &board) {
        return Err(ScanFail::Pointer);
    }

    let mut presses = 0usize;

    loop {
        let Some((_, frame)) = window_frame(window, screen, id) else {
            return Err(ScanFail::NoGradient);
        };
        let label = label::read(&frame, origin);
        log(&format!("    button reads {}", label.name()));

        let grid = match label {
            label::Label::PickColor => {
                if presses >= MAX_PRESSES {
                    log("    two presses and still no colour chosen - stopping");
                    return Err(ScanFail::NoLegend);
                }
                if !mouse.move_to(button_row.0, button_row.1) {
                    return Err(ScanFail::Pointer);
                }
                sleep(Duration::from_millis(200));
                mouse.click();
                presses += 1;
                sleep(Duration::from_millis(420));
                if opts.halted() {
                    return Err(ScanFail::Pointer);
                }
                // Grab before the pointer goes onto the grid: the legend is drawn
                // at the pointer, and the searched image must not contain it.
                park(mouse, &board);
                settled_grab(window, screen, id, origin, log).ok_or(ScanFail::NoGradient)?
            }
            label::Label::StartDyeing => {
                settled_grab(window, screen, id, origin, log).ok_or(ScanFail::NoGradient)?
            }
            label::Label::Unknown => {
                log("    the button label is unreadable - not pressing anything");
                return Err(ScanFail::ButtonUnknown);
            }
        };

        match read_offsets(window, screen, mouse, id, &board, log) {
            Ok((table, pointer, boxes)) => {
                return Ok(Prepared {
                    board,
                    grid,
                    pointer,
                    boxes,
                    table,
                    armed: presses > 0,
                })
            }
            Err(ScanFail::NoLegend) if label == label::Label::PickColor => continue,
            Err(other) => return Err(other),
        }
    }
}

/// Prepare one search and report the best candidate, without clicking anything.
pub fn scan_once(
    window: &dyn GameWindow,
    screen: &dyn Screen,
    mouse: &dyn Mouse,
    opts: &Options,
) -> Result<Scan, ScanFail> {
    let id = window.find().ok_or(ScanFail::NoClient)?;
    let prep = prepare(window, screen, mouse, id, opts, &mut |_: &str| {})?;
    let (colours, _) = wanted(opts, prep.table.len());
    let best = search::search_with(&prep.grid, &prep.table, &colours, opts.variance);

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

/// Discard the chosen colour, and confirm that the client took the click.
///
/// Cancel is only safe while the button reads "Start Dyeing", which `prepare`
/// has just established from the legend.  The click is repeated once because a
/// click sent the instant the pointer lands is sometimes swallowed; the label is
/// read back after each try, and a button that still reads "Start Dyeing" means
/// the colour is still chosen.
fn cancel_roll(
    window: &dyn GameWindow,
    screen: &dyn Screen,
    mouse: &dyn Mouse,
    id: WindowId,
    board: &Board,
    opts: &Options,
    log: &mut dyn FnMut(&str),
) -> bool {
    for _ in 0..2 {
        let (cx, cy) = button(board, opts.cancel_x, opts.button_y);
        if !mouse.move_to(cx, cy) {
            return false;
        }
        // Let the client register the hover before the button press.
        sleep(Duration::from_millis(220));
        mouse.click();
        sleep(SETTLE);
        if opts.halted() {
            return false;
        }

        let Some((_, frame)) = window_frame(window, screen, id) else {
            return false;
        };
        let label = label::read(&frame, board.origin);
        log(&format!("    after cancel the button reads {}", label.name()));
        if label != label::Label::StartDyeing {
            return true;
        }
    }
    false
}

/// The colours to look for, one entry per measured sample, and how many of them
/// must hold it.  A chosen slot takes priority over the slot count.
fn wanted(opts: &Options, len: usize) -> (Vec<Option<Rgb>>, usize) {
    search::plan(len, opts.colour, opts.slots, opts.prefer)
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

    // Switch to the client before looking for anything. Its palette is drawn on
    // its own window, so starting from a background window only burned a full
    // locate timeout before the switch that was going to happen anyway.
    report("  switching to the client");
    window.focus(id);

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

        let prep = match prepare(window, screen, mouse, id, opts, &mut |line: &str| report(line)) {
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
            Err(ScanFail::ButtonUnknown) => {
                report(&format!(
                    "cycle {cycles}: cannot read the dye button - stopping without pressing it"
                ));
                return Outcome::NoClient;
            }
        };

        if prep.armed {
            report("  pressed Pick Color to arm the picker");
        }
        report(&format!(
            "cycle {cycles}: {} slots (pointer {},{})",
            prep.table.len(),
            prep.pointer.0,
            prep.pointer.1
        ));

        let (colours, need) = wanted(opts, prep.table.len());
        if let Some(best) = search::search_with(&prep.grid, &prep.table, &colours, opts.variance) {
            match opts.prefer.filter(|index| *index < prep.table.len()) {
                Some(index) => report(&format!(
                    "  best=({},{}) slot {} holds it",
                    best.x,
                    best.y,
                    index + 1
                )),
                None => report(&format!(
                    "  best=({},{}) {}/{} need={need}",
                    best.x,
                    best.y,
                    best.count,
                    prep.table.len()
                )),
            }

            if best.count >= need && prep.table.len() >= MIN_SLOTS {
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

        // Reaching here means the legend was live, so the button reads "Start
        // Dyeing": Cancel discards the colour and rolls a new one, which is the
        // only state in which that click is safe.
        report("  no place holds enough of the colour - cancelling");
        window.focus(id);
        if !cancel_roll(window, screen, mouse, id, &prep.board, opts, &mut |line: &str| {
            report(line)
        }) {
            report("  the cancel click did not take - stopping");
            return Outcome::Exhausted { cycles };
        }
    }

    Outcome::Exhausted { cycles }
}

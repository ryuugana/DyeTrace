//! The buttons of the client's dye window are dangerous to press blind.
//!
//! "Pick Color" (white label) arms the picker. Once a colour is chosen the same
//! button reads "Start Dyeing" in grey and applies the dye, and Cancel discards
//! that choice. Cancel while the label still says "Pick Color" closes the dye
//! window - which is how an attempt was destroyed by a "reset the button" click.
//!
//! These tests drive a client stand-in that behaves like the real one: the label
//! follows the state, the legend only appears while a colour is being picked and
//! the pointer is over the grid, and Cancel while idle closes the window.

use std::cell::{Cell, RefCell};

use dyetrace_core::image::{Image, Rgb};
use dyetrace_core::label;
use dyetrace_core::{CAP_H, CAP_W};
use dyetrace_engine::cycle::{self, Options};
use dyetrace_platform::traits::{GameWindow, Mouse, Rect, Screen, WindowId};

const FRAME_W: usize = 320;
const FRAME_H: usize = 480;
/// Where the palette sits inside the window.
const PAL: (i32, i32) = (32, 40);
/// The client's button offsets, from the palette origin.
const PICK: (i32, i32) = (147, 397);
const CANCEL: (i32, i32) = (231, 397);
/// Sample offsets the stand-in draws, measured on the live client.
const OFFSETS: [(i32, i32); 5] = [(-47, -48), (29, -45), (0, 0), (-44, 23), (26, 30)];

fn pick_button() -> (i32, i32) {
    (PAL.0 + PICK.0, PAL.1 + PICK.1)
}

fn cancel_button() -> (i32, i32) {
    (PAL.0 + CANCEL.0, PAL.1 + CANCEL.1)
}

fn near(pos: (i32, i32), button: (i32, i32)) -> bool {
    (pos.0 - button.0).abs() <= 4 && (pos.1 - button.1).abs() <= 4
}

/// A window grab: gradient palette, the button's label, and - only while armed
/// and hovering the grid - the client's legend.
fn draw(armed: bool, pointer: (i32, i32), _roll: u8) -> Image {
    let mut img = Image::filled(FRAME_W, FRAME_H, Rgb::new(0x60, 0x60, 0x60));
    for y in 0..CAP_H as i32 {
        for x in 0..CAP_W as i32 {
            let r = (x as u8).wrapping_mul(3);
            let g = (y as u8).wrapping_mul(3);
            img.put_i(PAL.0 + x, PAL.1 + y, Rgb::new(r, g, 0x80));
        }
    }

    // The white face reads about forty pixels above luminance 130, the grey one
    // none - the margin the label reader relies on.
    let glyph = if armed { 0x70u8 } else { 0xF0 };
    let lx = PAL.0 + label::LABEL_X;
    let ly = PAL.1 + label::LABEL_Y;
    for i in 0..46 {
        let x = lx + (i % 23);
        let y = ly + 10 + (i / 23);
        img.put_i(x, y, Rgb::new(glyph, glyph, glyph));
    }

    let over_grid = (0..CAP_W as i32).contains(&pointer.0) && (0..CAP_H as i32).contains(&pointer.1);
    if armed && over_grid {
        for (i, (dx, dy)) in OFFSETS.iter().enumerate() {
            let fill = Rgb::new(0x30 + i as u8 * 20, 0x90, 0x60);
            // The box's centre sits 10 px right of its crosshair, and the ring
            // is 8 px across, so its top-left corner is half a box up and left.
            legend_box(
                &mut img,
                PAL.0 + pointer.0 + dx + 10 - 4,
                PAL.1 + pointer.1 + dy - 4,
                fill,
            );
        }
    }
    img
}

fn legend_box(img: &mut Image, x: i32, y: i32, fill: Rgb) {
    const N: i32 = 8;
    let ring = Rgb::new(0, 0, 0);
    for i in 0..N {
        img.put_i(x + i, y, ring);
        img.put_i(x + i, y + N - 1, ring);
        img.put_i(x, y + i, ring);
        img.put_i(x + N - 1, y + i, ring);
    }
    for j in 1..N - 1 {
        for i in 1..N - 1 {
            img.put_i(x + i, y + j, fill);
        }
    }
}

/// A client window that reacts to clicks the way the real one does.
struct Client {
    armed: Cell<bool>,
    closed: Cell<bool>,
    roll: Cell<u8>,
    x: Cell<i32>,
    y: Cell<i32>,
    clicks: RefCell<Vec<(i32, i32)>>,
    focus_calls: Cell<u32>,
}

impl Client {
    fn new(armed: bool) -> Self {
        Self {
            armed: Cell::new(armed),
            closed: Cell::new(false),
            roll: Cell::new(0),
            x: Cell::new(0),
            y: Cell::new(0),
            clicks: RefCell::new(Vec::new()),
            focus_calls: Cell::new(0),
        }
    }

    fn clicks(&self) -> Vec<(i32, i32)> {
        self.clicks.borrow().clone()
    }

    fn at_button(&self, button: (i32, i32)) -> usize {
        self.clicks().iter().filter(|c| near(**c, button)).count()
    }

    fn frame(&self) -> Image {
        if self.closed.get() {
            return Image::filled(FRAME_W, FRAME_H, Rgb::new(0, 0, 0));
        }
        let pointer = (self.x.get() - PAL.0, self.y.get() - PAL.1);
        draw(self.armed.get(), pointer, self.roll.get())
    }
}

impl Screen for Client {
    fn screen_size(&self) -> (i32, i32) {
        (FRAME_W as i32, FRAME_H as i32)
    }

    fn capture(&self, x: i32, y: i32, w: u32, h: u32) -> Option<Image> {
        self.frame().sub_image(x, y, w as usize, h as usize)
    }
}

impl Mouse for Client {
    fn position(&self) -> (i32, i32) {
        (self.x.get(), self.y.get())
    }

    fn move_to(&self, x: i32, y: i32) -> bool {
        self.x.set(x);
        self.y.set(y);
        true
    }

    fn click(&self) {
        let pos = (self.x.get(), self.y.get());
        self.clicks.borrow_mut().push(pos);
        if near(pos, pick_button()) {
            self.armed.set(true);
            self.roll.set(self.roll.get().wrapping_add(7));
        } else if near(pos, cancel_button()) {
            if self.armed.get() {
                self.armed.set(false);
                self.roll.set(self.roll.get().wrapping_add(11));
            } else {
                // The real client closes the dye window here.
                self.closed.set(true);
            }
        }
    }

    fn nudge(&self, dx: i32, dy: i32) {
        self.x.set(self.x.get() + dx);
        self.y.set(self.y.get() + dy);
    }
}

impl GameWindow for Client {
    fn find(&self) -> Option<WindowId> {
        Some(WindowId(1))
    }

    fn rect(&self, _id: WindowId) -> Option<Rect> {
        Some(Rect {
            x: 0,
            y: 0,
            w: FRAME_W as i32,
            h: FRAME_H as i32,
        })
    }

    fn focus(&self, _id: WindowId) -> bool {
        self.focus_calls.set(self.focus_calls.get() + 1);
        true
    }
}

#[test]
fn an_idle_window_is_armed_with_one_pick_color_press() {
    let client = Client::new(false);

    let scan = cycle::scan_once(&client, &client, &client, &Options::default())
        .expect("one press should arm the picker and reveal the legend");

    assert!(scan.armed, "the picker had to be armed");
    assert_eq!(
        scan.table[0],
        (0, 0),
        "slot 1 is the pointer, got {:?}",
        scan.table
    );
    assert_eq!(client.at_button(pick_button()), 1, "clicks {:?}", client.clicks());
    assert_eq!(client.at_button(cancel_button()), 0, "Cancel was never needed");
    assert!(!client.closed.get(), "the dye window must still be open");
    assert!(scan.table.len() >= 4, "table was {:?}", scan.table);
}

#[test]
fn every_chosen_slot_must_hold_the_colour() {
    let client = Client::new(true);
    // The stand-in's gradient is (3x, 3y, 0x80), so this colour exists at one
    // position only: (27, 27).
    let colour = Rgb::new(0x51, 0x51, 0x80);

    let one = cycle::scan_once(
        &client,
        &client,
        &client,
        &Options {
            colour,
            slots_only: vec![0],
            ..Options::default()
        },
    )
    .expect("scan");
    assert_eq!(
        one.best.map(|best| (best.x, best.y)),
        Some((27, 27)),
        "slot 1 is the pointer's own sample"
    );

    // Add slot 2, which samples 47 px up and left: no position holds the colour
    // in both samples, so nothing can satisfy the pair.
    let two = cycle::scan_once(
        &client,
        &client,
        &client,
        &Options {
            colour,
            slots_only: vec![0, 1],
            ..Options::default()
        },
    )
    .expect("scan");
    assert!(
        two.best.map(|best| best.count).unwrap_or(0) < 2,
        "both slots cannot hold it, got {:?}",
        two.best
    );
}

#[test]
fn an_armed_window_is_used_without_pressing_anything() {    let client = Client::new(true);

    let scan = cycle::scan_once(&client, &client, &client, &Options::default())
        .expect("a live legend needs no arming");

    assert!(!scan.armed, "nothing had to be pressed");
    assert!(client.clicks().is_empty(), "clicks {:?}", client.clicks());
    assert!(!client.closed.get());
}

#[test]
fn the_client_is_raised_before_anything_is_looked_for() {
    // A closed client: nothing can be located, so the only evidence of ordering
    // is that the client was raised before the search began.
    let client = Client::new(false);
    client.closed.set(true);

    let outcome = cycle::run(&client, &client, &client, &Options::default(), |_| {});
    assert!(matches!(outcome, cycle::Outcome::NoClient));
    assert!(
        client.focus_calls.get() >= 1,
        "auto must switch to the client before searching for the palette"
    );
    assert!(client.clicks().is_empty(), "nothing may be clicked while blind");
}

#[test]
fn a_colour_that_never_matches_cancels_and_keeps_the_window() {
    let client = Client::new(false);
    let opts = Options {
        slots: 5,
        max_cycles: 2,
        ..Options::default()
    };

    let mut lines = Vec::new();
    let outcome = cycle::run(&client, &client, &client, &opts, |line| lines.push(line.to_string()));

    assert!(
        matches!(outcome, cycle::Outcome::Exhausted { cycles: 2 }),
        "unexpected outcome {outcome:?}\n{lines:#?}"
    );
    assert!(!client.closed.get(), "the dye window must survive the cycle");
    assert_eq!(
        client.at_button(pick_button()),
        2,
        "one arming press per roll, clicks {:?}",
        client.clicks()
    );
    // Cancel is only safe once a colour has been chosen, and the label proves it.
    assert_eq!(
        client.at_button(cancel_button()),
        2,
        "clicks {:?}",
        client.clicks()
    );
}

/// A screen whose button label is neither face of the real one.
struct DimLabel;

impl Screen for DimLabel {
    fn screen_size(&self) -> (i32, i32) {
        (FRAME_W as i32, FRAME_H as i32)
    }

    fn capture(&self, x: i32, y: i32, w: u32, h: u32) -> Option<Image> {
        let mut img = Image::filled(FRAME_W, FRAME_H, Rgb::new(0x60, 0x60, 0x60));
        for y in 0..CAP_H as i32 {
            for x in 0..CAP_W as i32 {
                img.put_i(
                    PAL.0 + x,
                    PAL.1 + y,
                    Rgb::new((x * 3) as u8, (y * 3) as u8, 0x80),
                );
            }
        }
        let lx = PAL.0 + label::LABEL_X;
        let ly = PAL.1 + label::LABEL_Y;
        for i in 0..5 {
            img.put_i(lx + i, ly + 10, Rgb::new(0xF0, 0xF0, 0xF0));
        }
        img.sub_image(x, y, w as usize, h as usize)
    }
}

#[test]
fn an_unreadable_button_is_never_pressed() {
    // The engine must leave the window alone rather than guess at an unclear
    // label - a wrong guess costs the dye attempt.
    let client = Client::new(false);
    let result = cycle::scan_once(&client, &DimLabel, &client, &Options::default());
    assert_eq!(result.unwrap_err(), cycle::ScanFail::ButtonUnknown);
    assert!(client.clicks().is_empty(), "clicks {:?}", client.clicks());
}

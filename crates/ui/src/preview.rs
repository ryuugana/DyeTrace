//! A live view of the dye palette.
//!
//! One capture path feeds both the picture on screen and the search: every frame
//! the engine grabs goes through [`Capture`], which publishes the palette it
//! contains.  A thread of its own does the same every half second, so the
//! picture is current even when nothing is being scanned.
//!
//! A colour being picked freezes the view.  While the button reads "Start
//! Dyeing" the tick stops publishing, because a legend drawn over the grid would
//! end up in the picture; Cancel brings "Pick Color" back and the view resumes.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use std::sync::atomic::{AtomicBool, Ordering};

use dyetrace_core::image::{self, Image};
use dyetrace_core::label::{self, Label};
use dyetrace_core::{markers, palette, CAP_H, CAP_W, MAX_TARGETS};
use dyetrace_engine::cycle::MIN_GRADIENT;
use dyetrace_platform::traits::{GameWindow, Screen, WindowId};
use dyetrace_platform_win::{WinScreen, WinWindow};

/// How often the idle palette is captured.
const TICK: Duration = Duration::from_millis(500);
/// How far two signatures may drift and still be the same roll of colours.
const SIGNATURE_SLACK: u8 = 8;

/// The palette as it was last seen.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub origin: (i32, i32),
    /// The 256x256 palette, without any legend.
    pub grid: Image,
    /// Its fingerprint, so a scan can tell whether its offsets still apply.
    pub signature: Vec<u8>,
}

impl Snapshot {
    /// Is a scan's gradient the one still on screen?
    pub fn shows(&self, other: &[u8]) -> bool {
        image::same_signature(&self.signature, other, SIGNATURE_SLACK)
    }
}

struct Shared {
    latest: Mutex<Option<Snapshot>>,
}

/// The window's view of the palette, kept fresh by a thread of its own.
pub struct Preview {
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
}

impl Preview {
    pub fn spawn() -> Self {
        let shared = Arc::new(Shared {
            latest: Mutex::new(None),
        });
        let stop = Arc::new(AtomicBool::new(false));

        let ticking = Arc::clone(&shared);
        let ending = Arc::clone(&stop);
        thread::spawn(move || tick(ticking, ending));

        Self { shared, stop }
    }

    /// The capture path the engine should use, so its frames reach the view.
    pub fn capture(&self) -> Capture {
        Capture {
            screen: WinScreen::new(),

            shared: Arc::clone(&self.shared),
            origin: Mutex::new(None),
        }
    }

    pub fn latest(&self) -> Option<Snapshot> {
        self.shared
            .latest
            .lock()
            .ok()
            .and_then(|slot| slot.clone())
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn tick(shared: Arc<Shared>, stop: Arc<AtomicBool>) {
    let screen = WinScreen::new();
    let window = WinWindow::mabinogi();

    while !stop.load(Ordering::Relaxed) {
        thread::sleep(TICK);

        let Some(id) = window.find() else {
            clear(&shared);
            continue;
        };
        let Some(frame) = Screen::capture_window(&screen, id) else {
            continue;
        };

        let cached = shared
            .latest
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(|snapshot| snapshot.origin));
        let Some(origin) = origin_for(&frame, cached) else {
            clear(&shared);
            continue;
        };

        // A colour is being picked: hold the last picture until it is done.
        if label::read(&frame, origin) == Label::StartDyeing {
            continue;
        }

        match snapshot(origin, &frame) {
            Some(snapshot) => publish(&shared, snapshot),
            None => clear(&shared),
        }
    }
}

fn clear(shared: &Shared) {
    if let Ok(mut slot) = shared.latest.lock() {
        *slot = None;
    }
}

fn publish(shared: &Shared, snapshot: Snapshot) {
    if let Ok(mut slot) = shared.latest.lock() {
        *slot = Some(snapshot);
    }
}

/// Where the palette is: the cached corner if it still holds a palette,
/// otherwise a fresh search of the frame.
fn origin_for(frame: &Image, cached: Option<(i32, i32)>) -> Option<(i32, i32)> {
    if let Some(origin) = cached {
        if let Some(grid) = frame.sub_image(origin.0, origin.1, CAP_W, CAP_H) {
            if palette::gradient_percent(&grid) >= MIN_GRADIENT {
                return Some(origin);
            }
        }
    }
    palette::find_square_field(frame).or_else(|| palette::find_field(frame))
}

/// Cut the palette out of a frame, unless it holds something that is not the
/// gradient - a legend drawn over it, or no palette at all.
fn snapshot(origin: (i32, i32), frame: &Image) -> Option<Snapshot> {
    let grid = frame.sub_image(origin.0, origin.1, CAP_W, CAP_H)?;
    if palette::gradient_percent(&grid) < MIN_GRADIENT {
        return None;
    }
    if !markers::detect_boxes(&grid, MAX_TARGETS).is_empty() {
        return None;
    }

    Some(Snapshot {
        origin,
        signature: grid.signature(),
        grid,
    })
}

/// The screen the rest of the program captures through: every frame it returns
/// is also offered to the view.
pub struct Capture {
    screen: WinScreen,

    shared: Arc<Shared>,
    origin: Mutex<Option<(i32, i32)>>,
}

impl Capture {
    fn publish(&self, frame: &Image) {
        let cached = self.origin.lock().ok().and_then(|slot| *slot);
        let Some(origin) = origin_for(frame, cached) else {
            return;
        };
        if let Ok(mut slot) = self.origin.lock() {
            *slot = Some(origin);
        }

        if let Some(snapshot) = snapshot(origin, frame) {
            publish(&self.shared, snapshot);
        }
    }
}

impl Screen for Capture {
    fn screen_size(&self) -> (i32, i32) {
        self.screen.screen_size()
    }

    fn capture(&self, x: i32, y: i32, w: u32, h: u32) -> Option<Image> {
        self.screen.capture(x, y, w, h)
    }

    fn capture_window(&self, id: WindowId) -> Option<Image> {
        let frame = Screen::capture_window(&self.screen, id)?;
        self.publish(&frame);
        Some(frame)
    }
}

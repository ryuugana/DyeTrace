//! A screen and pointer that replay saved frames, so the pipeline can be
//! exercised without a client.

use crate::bmp;
use crate::traits::{Mouse, Screen};
use dyetrace_core::image::Image;
use std::cell::Cell;
use std::io;
use std::path::Path;

/// Answers [`Screen::capture`] from a fixed frame.
pub struct FakeScreen {
    frame: Image,
}

impl FakeScreen {
    pub fn new(frame: Image) -> Self {
        Self { frame }
    }

    pub fn from_bmp(path: &Path) -> io::Result<Self> {
        Ok(Self::new(bmp::read_bmp(path)?))
    }

    pub fn frame(&self) -> &Image {
        &self.frame
    }

    pub fn set_frame(&mut self, frame: Image) {
        self.frame = frame;
    }
}

impl Screen for FakeScreen {
    fn screen_size(&self) -> (i32, i32) {
        (self.frame.width() as i32, self.frame.height() as i32)
    }

    fn capture(&self, x: i32, y: i32, w: u32, h: u32) -> Option<Image> {
        self.frame.sub_image(x, y, w as usize, h as usize)
    }
}

/// A pointer that remembers where it was put and how often it was clicked.
pub struct FakeMouse {
    x: Cell<i32>,
    y: Cell<i32>,
    clicks: Cell<u32>,
    /// When false, [`Mouse::move_to`] reports failure, simulating a client that
    /// ignores injected movement.
    pub accepts_moves: bool,
}

impl FakeMouse {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x: Cell::new(x),
            y: Cell::new(y),
            clicks: Cell::new(0),
            accepts_moves: true,
        }
    }

    pub fn clicks(&self) -> u32 {
        self.clicks.get()
    }
}

impl Mouse for FakeMouse {
    fn position(&self) -> (i32, i32) {
        (self.x.get(), self.y.get())
    }

    fn move_to(&self, x: i32, y: i32) -> bool {
        if !self.accepts_moves {
            return false;
        }
        self.x.set(x);
        self.y.set(y);
        true
    }

    fn click(&self) {
        self.clicks.set(self.clicks.get() + 1);
    }

    fn nudge(&self, dx: i32, dy: i32) {
        if self.accepts_moves {
            self.x.set(self.x.get() + dx);
            self.y.set(self.y.get() + dy);
        }
    }
}

//! A screen and pointer that replay saved frames, so the pipeline can be
//! exercised without a client.

use crate::bmp;
use crate::traits::{GameWindow, Mouse, Rect, Screen, WindowId};
use dyetrace_core::image::Image;
use std::cell::{Cell, RefCell};
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

/// A pointer that remembers where it was put and every click it was given.
pub struct FakeMouse {
    x: Cell<i32>,
    y: Cell<i32>,
    clicks: RefCell<Vec<(i32, i32)>>,
    /// When false, [`Mouse::move_to`] reports failure, simulating a client that
    /// ignores injected movement.
    pub accepts_moves: bool,
}

impl FakeMouse {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x: Cell::new(x),
            y: Cell::new(y),
            clicks: RefCell::new(Vec::new()),
            accepts_moves: true,
        }
    }

    /// Where each click landed, in the order they were sent.
    pub fn clicks(&self) -> Vec<(i32, i32)> {
        self.clicks.borrow().clone()
    }

    pub fn click_count(&self) -> usize {
        self.clicks.borrow().len()
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
        let pos = (self.x.get(), self.y.get());
        self.clicks.borrow_mut().push(pos);
    }

    fn nudge(&self, dx: i32, dy: i32) {
        if self.accepts_moves {
            self.x.set(self.x.get() + dx);
            self.y.set(self.y.get() + dy);
        }
    }
}

/// A client window at a fixed place that counts how often it was raised.
pub struct FakeWindow {
    rect: Rect,
    id: WindowId,
    focus_calls: Cell<u32>,
}

impl FakeWindow {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            id: WindowId(1),
            focus_calls: Cell::new(0),
        }
    }

    pub fn focus_calls(&self) -> u32 {
        self.focus_calls.get()
    }
}

impl GameWindow for FakeWindow {
    fn find(&self) -> Option<WindowId> {
        Some(self.id)
    }

    fn rect(&self, _id: WindowId) -> Option<Rect> {
        Some(self.rect)
    }

    fn focus(&self, _id: WindowId) -> bool {
        self.focus_calls.set(self.focus_calls.get() + 1);
        true
    }
}

//! The platform boundary.

use dyetrace_core::image::Image;

/// A window rectangle in screen coordinates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Opaque handle to a client window.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WindowId(pub usize);

/// Reads pixels off the screen.
pub trait Screen {
    /// Size of the whole screen, in pixels.
    fn screen_size(&self) -> (i32, i32);

    /// Grab a rectangle.  Returns `None` if it is not fully on screen.
    fn capture(&self, x: i32, y: i32, w: u32, h: u32) -> Option<Image>;
}

/// Moves and clicks the pointer.
pub trait Mouse {
    /// Where the pointer is, in screen coordinates.
    fn position(&self) -> (i32, i32);

    /// Move the pointer.  Returns true only if it really arrived: callers must
    /// not click when this fails.
    fn move_to(&self, x: i32, y: i32) -> bool;

    /// Left click at the current position.
    fn click(&self);

    /// Optional relative motion, used to make a client re-evaluate its hover
    /// state.  Backends that cannot move relatively may leave this empty.
    fn nudge(&self, _dx: i32, _dy: i32) {}
}

/// Finds and raises the client window.
pub trait GameWindow {
    /// The client window, if it is on screen.
    fn find(&self) -> Option<WindowId>;

    /// The client's rectangle.
    fn rect(&self, id: WindowId) -> Option<Rect>;

    /// Bring the client to the foreground.
    fn focus(&self, id: WindowId) -> bool;
}

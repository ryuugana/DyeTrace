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

    /// Grab a whole window even when it is partly covered, so reading a client
    /// does not require raising it.  Backends without such a call return `None`
    /// and callers fall back to [`Screen::capture`].
    fn capture_window(&self, _id: WindowId) -> Option<Image> {
        None
    }

    /// Where the palette was last seen, if the backend keeps track of it.
    ///
    /// A hint saves searching a captured frame for a square border.  It is only
    /// a hint: whoever uses it still checks the palette is there.
    fn origin_hint(&self) -> Option<(i32, i32)> {
        None
    }
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

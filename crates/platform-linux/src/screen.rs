//! Screen capture through `GetImage`.
//!
//! A rectangle is read from the root window in one request on a normal X11
//! desktop.  A whole-window capture reads the client's own drawable instead,
//! which on a composited desktop (including XWayland) still holds the window's
//! pixels when another window is on top of it.
//!
//! Rootless XWayland is the exception: its root window has no backing pixmap,
//! so `GetImage` on the root fails.  In that case a rectangle is assembled from
//! the visible top-level windows, painted in stacking order.  The game is read
//! through [`Screen::capture_window`] either way, so the fallback only affects
//! `--snapshot` and the probe.

use crate::connection::X11;
use dyetrace_core::image::{Image, Rgb};
use dyetrace_platform::traits::{Screen, WindowId};
use x11rb::protocol::xproto::{ConnectionExt, MapState, Window, WindowClass};

/// Reads pixels off the X11 desktop.
pub struct X11Screen {
    x11: Option<X11>,
}

impl Default for X11Screen {
    fn default() -> Self {
        Self::new()
    }
}

impl X11Screen {
    pub fn new() -> Self {
        Self {
            x11: X11::connect(),
        }
    }

    /// Whether a display was found.  Calls quietly do nothing without one.
    pub fn connected(&self) -> bool {
        self.x11.is_some()
    }

    /// Grab a whole window, relative to its own top-left corner.
    pub fn window_image(&self, id: WindowId) -> Option<Image> {
        let x11 = self.x11.as_ref()?;
        let window = id.0 as Window;
        let geometry = x11.conn.get_geometry(window).ok()?.reply().ok()?;
        x11.image(window, 0, 0, geometry.width, geometry.height)
    }

    /// Assemble a rectangle from the visible top-level windows.
    ///
    /// Children of the root are listed bottom to top, so painting them in that
    /// order lets the topmost window win where they overlap.  Anything the
    /// windows do not cover stays black, the same as an empty desktop.
    fn composite(&self, x: i32, y: i32, w: u32, h: u32) -> Option<Image> {
        let x11 = self.x11.as_ref()?;
        let tree = x11.conn.query_tree(x11.root).ok()?.reply().ok()?;
        let mut canvas = Image::filled(w as usize, h as usize, Rgb::black());

        for child in tree.children {
            let Some(geometry) = x11
                .conn
                .get_geometry(child)
                .ok()
                .and_then(|c| c.reply().ok())
            else {
                continue;
            };
            if geometry.width == 0 || geometry.height == 0 {
                continue;
            }

            let Some(attributes) = x11
                .conn
                .get_window_attributes(child)
                .ok()
                .and_then(|c| c.reply().ok())
            else {
                continue;
            };
            // Only a viewable input/output window has pixels to read.
            if attributes.class != WindowClass::INPUT_OUTPUT
                || attributes.map_state != MapState::VIEWABLE
            {
                continue;
            }

            let Some(translated) = x11
                .conn
                .translate_coordinates(child, x11.root, 0, 0)
                .ok()
                .and_then(|c| c.reply().ok())
            else {
                continue;
            };

            let left = translated.dst_x as i32;
            let top = translated.dst_y as i32;
            let right = left + geometry.width as i32;
            let bottom = top + geometry.height as i32;

            // Intersect the window with the requested rectangle.
            let clip_left = left.max(x);
            let clip_top = top.max(y);
            let clip_right = right.min(x + w as i32);
            let clip_bottom = bottom.min(y + h as i32);
            if clip_left >= clip_right || clip_top >= clip_bottom {
                continue;
            }

            let cw = (clip_right - clip_left) as u16;
            let ch = (clip_bottom - clip_top) as u16;
            let sx = (clip_left - left) as i16;
            let sy = (clip_top - top) as i16;
            let Some(piece) = x11.image(child, sx, sy, cw, ch) else {
                continue;
            };

            let dx = (clip_left - x) as usize;
            let dy = (clip_top - y) as usize;
            let dst_stride = w as usize;
            let src_stride = cw as usize;
            let dst = canvas.as_mut_slice();
            let src = piece.as_slice();
            for row in 0..ch as usize {
                let d = (dy + row) * dst_stride + dx;
                let s = row * src_stride;
                dst[d..d + src_stride].copy_from_slice(&src[s..s + src_stride]);
            }
        }

        Some(canvas)
    }
}

impl Screen for X11Screen {
    fn screen_size(&self) -> (i32, i32) {
        match &self.x11 {
            Some(x11) => (x11.width, x11.height),
            None => (0, 0),
        }
    }

    fn capture(&self, x: i32, y: i32, w: u32, h: u32) -> Option<Image> {
        if w == 0 || h == 0 {
            return None;
        }
        let x11 = self.x11.as_ref()?;
        if !x11.on_screen(x, y, w, h) {
            return None;
        }
        x11.image(x11.root, x as i16, y as i16, w as u16, h as u16)
            .or_else(|| self.composite(x, y, w, h))
    }

    fn capture_window(&self, id: WindowId) -> Option<Image> {
        self.window_image(id)
    }
}

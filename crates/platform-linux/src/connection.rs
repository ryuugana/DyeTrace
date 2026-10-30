//! The X11 connection and the screen's pixel format.
//!
//! One connection per backend object keeps the three traits independent, the
//! same way the Win32 backend opens its own device contexts.  A missing
//! `DISPLAY` is not fatal: every call reports "no pixels" instead of aborting,
//! so the CLI and the self-tests still run on a headless machine.

use dyetrace_core::image::Image;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat, ImageOrder, Visualtype, Window};
use x11rb::rust_connection::RustConnection;

use crate::pixels::{self, PixelFormat};

/// A live connection plus everything needed to read the root window.
pub struct X11 {
    pub conn: RustConnection,
    pub root: Window,
    pub width: i32,
    pub height: i32,
    pub format: PixelFormat,
}

impl X11 {
    /// Connect to the display named by `DISPLAY`, or `None` when there is no
    /// X server to talk to.
    pub fn connect() -> Option<Self> {
        let (conn, screen_num) = x11rb::connect(None).ok()?;

        // Everything borrowed from `setup` is copied out here, so the
        // connection can be moved into the returned value.
        let (root, width, height, mut pixel_format) = {
            let setup = conn.setup();
            let screen = setup.roots.get(screen_num)?;
            let format = setup
                .pixmap_formats
                .iter()
                .find(|format| format.depth == screen.root_depth)?;
            let visual = root_visual(screen, screen.root_visual);

            let format = PixelFormat {
                bits_per_pixel: format.bits_per_pixel,
                scanline_pad: format.scanline_pad,
                lsb_first: setup.image_byte_order == ImageOrder::LSB_FIRST,
                red_mask: visual.map(|v| v.red_mask).unwrap_or(0),
                green_mask: visual.map(|v| v.green_mask).unwrap_or(0),
                blue_mask: visual.map(|v| v.blue_mask).unwrap_or(0),
            };

            (
                screen.root,
                screen.width_in_pixels as i32,
                screen.height_in_pixels as i32,
                format,
            )
        };

        // An 8-bit PseudoColor root has no channel masks.  There is nothing
        // sensible to show for it, so keep the standard masks and let the
        // capture succeed with the server's default TrueColor interpretation.
        if pixel_format.red_mask == 0 && pixel_format.green_mask == 0 && pixel_format.blue_mask == 0
        {
            pixel_format.red_mask = 0x00FF_0000;
            pixel_format.green_mask = 0x0000_FF00;
            pixel_format.blue_mask = 0x0000_00FF;
        }

        Some(Self {
            conn,
            root,
            width,
            height,
            format: pixel_format,
        })
    }

    /// Grab a rectangle from a drawable.  `x`/`y` are relative to it.
    pub fn image(&self, drawable: Window, x: i16, y: i16, w: u16, h: u16) -> Option<Image> {
        if w == 0 || h == 0 {
            return None;
        }

        let reply = self
            .conn
            .get_image(ImageFormat::Z_PIXMAP, drawable, x, y, w, h, !0)
            .ok()?
            .reply()
            .ok()?;
        let pixels = pixels::decode(&reply.data, w as usize, h as usize, &self.format)?;
        Some(Image::from_pixels(w as usize, h as usize, pixels))
    }

    /// Is a rectangle fully inside the root window?
    pub fn on_screen(&self, x: i32, y: i32, w: u32, h: u32) -> bool {
        x >= 0
            && y >= 0
            && x as i64 + w as i64 <= self.width as i64
            && y as i64 + h as i64 <= self.height as i64
            && w <= u16::MAX as u32
            && h <= u16::MAX as u32
            && x <= i16::MAX as i32
            && y <= i16::MAX as i32
    }
}

fn root_visual(screen: &x11rb::protocol::xproto::Screen, id: u32) -> Option<&Visualtype> {
    screen
        .allowed_depths
        .iter()
        .flat_map(|depth| depth.visuals.iter())
        .find(|visual| visual.visual_id == id)
}

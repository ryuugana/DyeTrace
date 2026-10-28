//! Screen capture through a DIB section.

use crate::ffi;
use dyetrace_core::image::Image;
use dyetrace_platform::traits::Screen;

/// Captures rectangles off the desktop.
#[derive(Default)]
pub struct WinScreen;

impl WinScreen {
    pub fn new() -> Self {
        Self
    }
}

pub const PW_RENDERFULLCONTENT: u32 = 2;

impl WinScreen {
    /// Capture a whole window even when it is partly covered, so reading the
    /// client never needs to steal focus from the interface.
    pub fn capture_window(&self, hwnd: isize) -> Option<Image> {
        if hwnd == 0 {
            return None;
        }
        unsafe {
            let mut rect = ffi::RECT::default();
            if ffi::GetWindowRect(hwnd, &mut rect) == 0 {
                return None;
            }
            let w = (rect.right - rect.left).max(1) as u32;
            let h = (rect.bottom - rect.top).max(1) as u32;
            self.capture_via(hwnd, w, h, true)
        }
    }

    fn capture_via(&self, hwnd: isize, w: u32, h: u32, print_window: bool) -> Option<Image> {
        unsafe {
            let screen = ffi::GetDC(0);
            let mem = ffi::CreateCompatibleDC(screen);
            let mut info: ffi::BITMAPINFO = core::mem::zeroed();
            info.header.size = core::mem::size_of::<ffi::BITMAPINFOHEADER>() as u32;
            info.header.width = w as i32;
            info.header.height = -(h as i32);
            info.header.planes = 1;
            info.header.bit_count = 32;
            info.header.compression = ffi::BI_RGB;
            let mut bits: *mut core::ffi::c_void = core::ptr::null_mut();
            let bitmap = ffi::CreateDIBSection(mem, &info, ffi::DIB_RGB_COLORS, &mut bits, 0, 0);
            let mut out = None;
            if bitmap != 0 && !bits.is_null() {
                let previous = ffi::SelectObject(mem, bitmap);
                let ok = if print_window {
                    ffi::PrintWindow(hwnd, mem, PW_RENDERFULLCONTENT) != 0
                } else {
                    ffi::BitBlt(mem, 0, 0, w as i32, h as i32, screen, 0, 0, ffi::SRCCOPY) != 0
                };
                if ok {
                    let pixels = core::slice::from_raw_parts(bits as *const u32, (w * h) as usize).to_vec();
                    out = Some(Image::from_pixels(w as usize, h as usize, pixels));
                }
                ffi::SelectObject(mem, previous);
                ffi::DeleteObject(bitmap);
            }
            ffi::DeleteDC(mem);
            ffi::ReleaseDC(0, screen);
            out
        }
    }
}

impl Screen for WinScreen {
    fn capture_window(&self, id: dyetrace_platform::traits::WindowId) -> Option<Image> {
        WinScreen::capture_window(self, id.0 as isize)
    }

    fn screen_size(&self) -> (i32, i32) {
        unsafe { (ffi::GetSystemMetrics(ffi::SM_CXSCREEN), ffi::GetSystemMetrics(ffi::SM_CYSCREEN)) }
    }

    fn capture(&self, x: i32, y: i32, w: u32, h: u32) -> Option<Image> {
        if w == 0 || h == 0 {
            return None;
        }

        // The trait promises nothing rather than garbage for a rectangle that
        // is not fully on screen; BitBlt would silently clip instead.
        let (screen_w, screen_h) = self.screen_size();
        if x < 0
            || y < 0
            || x as i64 + w as i64 > screen_w as i64
            || y as i64 + h as i64 > screen_h as i64
        {
            return None;
        }

        unsafe {
            let screen = ffi::GetDC(0);
            if screen == 0 {
                return None;
            }

            let mem = ffi::CreateCompatibleDC(screen);
            if mem == 0 {
                ffi::ReleaseDC(0, screen);
                return None;
            }

            let mut info: ffi::BITMAPINFO = core::mem::zeroed();
            info.header.size = core::mem::size_of::<ffi::BITMAPINFOHEADER>() as u32;
            info.header.width = w as i32;
            info.header.height = -(h as i32); // top-down, so rows arrive in order
            info.header.planes = 1;
            info.header.bit_count = 32;
            info.header.compression = ffi::BI_RGB;

            let mut bits: *mut core::ffi::c_void = core::ptr::null_mut();
            let bitmap = ffi::CreateDIBSection(mem, &info, ffi::DIB_RGB_COLORS, &mut bits, 0, 0);

            let mut grabbed = None;
            if bitmap != 0 && !bits.is_null() {
                let previous = ffi::SelectObject(mem, bitmap);
                if ffi::BitBlt(mem, 0, 0, w as i32, h as i32, screen, x, y, ffi::SRCCOPY) != 0 {
                    // BI_RGB 32 bpp in memory is B,G,R,X little-endian, which
                    // reads back as 0x00RRGGBB with the reserved byte zero.
                    let pixels =
                        core::slice::from_raw_parts(bits as *const u32, (w * h) as usize).to_vec();
                    grabbed = Some(Image::from_pixels(w as usize, h as usize, pixels));
                }
                ffi::SelectObject(mem, previous);
                ffi::DeleteObject(bitmap);
            }

            ffi::DeleteDC(mem);
            ffi::ReleaseDC(0, screen);
            grabbed
        }
    }
}

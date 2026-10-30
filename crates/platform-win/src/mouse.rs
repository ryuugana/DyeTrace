//! Pointer movement and clicking.

use crate::ffi;
use dyetrace_platform::traits::Mouse;
use std::thread::sleep;
use std::time::Duration;

/// Drives the real pointer.
pub struct WinMouse {
    /// Delay between button down and up.
    pub press_delay: Duration,
}

impl Default for WinMouse {
    fn default() -> Self {
        Self::new()
    }
}

impl WinMouse {
    pub fn new() -> Self {
        Self {
            press_delay: Duration::from_millis(30),
        }
    }
}

impl Mouse for WinMouse {
    fn position(&self) -> (i32, i32) {
        unsafe {
            let mut point = ffi::POINT::default();
            if ffi::GetCursorPos(&mut point) != 0 {
                (point.x, point.y)
            } else {
                (0, 0)
            }
        }
    }

    /// The client ignores a bare `SetCursorPos` for its own hover state, so an
    /// absolute move event is injected as well.  Returns false unless the
    /// pointer really arrived, so callers never click blind.
    fn move_to(&self, x: i32, y: i32) -> bool {
        let width = unsafe { ffi::GetSystemMetrics(ffi::SM_CXSCREEN) } - 1;
        let height = unsafe { ffi::GetSystemMetrics(ffi::SM_CYSCREEN) } - 1;

        for _ in 0..3 {
            unsafe {
                ffi::SetCursorPos(x, y);
                if width > 0 && height > 0 {
                    let ax = ((x as i64 * 65535) / width as i64) as u32;
                    let ay = ((y as i64 * 65535) / height as i64) as u32;
                    ffi::mouse_event(
                        ffi::MOUSEEVENTF_MOVE | ffi::MOUSEEVENTF_ABSOLUTE,
                        ax,
                        ay,
                        0,
                        0,
                    );
                }
            }
            sleep(Duration::from_millis(10));

            let (px, py) = self.position();
            if (px - x).abs() <= 1 && (py - y).abs() <= 1 {
                return true;
            }
        }

        false
    }

    fn click(&self) {
        unsafe {
            ffi::mouse_event(ffi::MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0);
            sleep(self.press_delay);
            ffi::mouse_event(ffi::MOUSEEVENTF_LEFTUP, 0, 0, 0, 0);
        }
    }

    /// Relative motion: `mouse_event` treats dx/dy as signed for a plain move.
    fn nudge(&self, dx: i32, dy: i32) {
        unsafe {
            ffi::mouse_event(ffi::MOUSEEVENTF_MOVE, dx as u32, dy as u32, 0, 0);
        }
    }
}

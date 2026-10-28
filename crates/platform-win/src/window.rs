//! Finding and raising the client window.

use crate::ffi;
use dyetrace_platform::traits::{GameWindow, Rect, WindowId};
use std::ffi::CString;
use std::thread::sleep;
use std::time::Duration;

/// Locates the client by window class and/or title.
pub struct WinWindow {
    class: Option<CString>,
    title: Option<CString>,
    /// How long to wait after raising the client before reading the screen.
    pub focus_delay: Duration,
}

impl WinWindow {
    /// The Mabinogi client registers a window of class "Mabinogi".
    pub fn mabinogi() -> Self {
        Self {
            class: CString::new("Mabinogi").ok(),
            title: None,
            focus_delay: Duration::from_millis(250),
        }
    }

    pub fn with_class(class: &str) -> Self {
        Self {
            class: CString::new(class).ok(),
            title: None,
            focus_delay: Duration::from_millis(250),
        }
    }
}

impl GameWindow for WinWindow {
    fn find(&self) -> Option<WindowId> {
        let class = self
            .class
            .as_ref()
            .map(|c| c.as_ptr() as *const u8)
            .unwrap_or(core::ptr::null());
        let title = self
            .title
            .as_ref()
            .map(|c| c.as_ptr() as *const u8)
            .unwrap_or(core::ptr::null());

        let hwnd = unsafe { ffi::FindWindowA(class, title) };
        if hwnd != 0 && unsafe { ffi::IsWindow(hwnd) } != 0 {
            Some(WindowId(hwnd as usize))
        } else {
            None
        }
    }

    fn rect(&self, id: WindowId) -> Option<Rect> {
        let hwnd = id.0 as ffi::HWND;
        unsafe {
            let mut r = ffi::RECT::default();
            if ffi::GetWindowRect(hwnd, &mut r) == 0 {
                return None;
            }
            Some(Rect {
                x: r.left,
                y: r.top,
                w: r.right - r.left,
                h: r.bottom - r.top,
            })
        }
    }

    /// Raise the client so clicks sent at screen coordinates reach it.
    ///
    /// A background process is not allowed to call `SetForegroundWindow`
    /// successfully, so `SwitchToThisWindow` is the fallback: it is exported by
    /// user32, needs no foreground rights, and unlike
    /// `SystemParametersInfo(SPI_SETFOREGROUNDLOCKTIMEOUT)` it does not
    /// broadcast a setting change that a busy client can block on.
    fn focus(&self, id: WindowId) -> bool {
        let hwnd = id.0 as ffi::HWND;

        unsafe {
            if ffi::GetForegroundWindow() == hwnd {
                return true;
            }

            ffi::SetForegroundWindow(hwnd);
            sleep(Duration::from_millis(80));
            if ffi::GetForegroundWindow() == hwnd {
                return true;
            }

            ffi::SwitchToThisWindow(hwnd, 1);
            sleep(self.focus_delay);
            ffi::GetForegroundWindow() == hwnd
        }
    }
}

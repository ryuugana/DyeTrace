#![cfg(windows)]
//! Win32 backend: the calls the client needs, behind the three platform traits.
//!
//! * [`WinScreen`] - `BitBlt` from the screen into a DIB section
//! * [`WinMouse`] - `SetCursorPos` plus an injected absolute move, then a
//!   read-back so a click is never sent to the wrong place
//! * [`WinWindow`] - `FindWindowA`, `GetWindowRect`, and foregrounding with the
//!   SPI lock-timeout trick the client needs

mod ffi;
mod mouse;
mod screen;
mod window;

pub use mouse::WinMouse;
pub use screen::WinScreen;
pub use window::WinWindow;

pub use dyetrace_platform::traits::{GameWindow, Mouse, Rect, Screen, WindowId};

/// Is a key down right now, wherever the keystroke is going?
///
/// `vk` is a virtual-key code, so this works even when another window - the
/// game - has the keyboard focus.
pub fn key_down(vk: i32) -> bool {
    // SAFETY: GetAsyncKeyState takes the key code by value and reads no memory
    // of ours; the high bit of the result is "down".
    unsafe { ffi::GetAsyncKeyState(vk) as u16 & 0x8000 != 0 }
}

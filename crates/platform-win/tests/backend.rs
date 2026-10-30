//! Smoke tests for the Win32 backend: every call the traits need, without
//! touching the client.

#![cfg(windows)]

use dyetrace_platform::traits::{GameWindow, Mouse, Screen};
use dyetrace_platform_win::{WinMouse, WinScreen, WinWindow};

#[test]
fn screen_reports_its_size() {
    let (w, h) = WinScreen::new().screen_size();
    assert!(w > 0 && h > 0, "screen size {w}x{h}");
}

#[test]
fn capture_returns_the_requested_rectangle() {
    let screen = WinScreen::new();
    let image = screen.capture(0, 0, 64, 48).expect("capture failed");
    assert_eq!(image.width(), 64);
    assert_eq!(image.height(), 48);
}

#[test]
fn capture_outside_the_screen_returns_nothing() {
    let (w, h) = WinScreen::new().screen_size();
    assert!(WinScreen::new().capture(w, h, 64, 48).is_none());
}

#[test]
fn pointer_position_is_on_screen() {
    let (x, y) = WinMouse::new().position();
    let (w, h) = WinScreen::new().screen_size();
    assert!(x >= 0 && y >= 0 && x <= w && y <= h, "pointer at {x},{y}");
}

#[test]
fn window_lookup_is_consistent() {
    let window = WinWindow::mabinogi();
    if let Some(id) = window.find() {
        let rect = window.rect(id).expect("rectangle for a live window");
        assert!(rect.w > 0 && rect.h > 0, "rect {rect:?}");
    }
}

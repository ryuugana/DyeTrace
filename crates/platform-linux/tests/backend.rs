//! Smoke tests for the X11 backend: every call the traits need, without
//! touching the client.
//!
//! They skip, rather than fail, when no display is reachable: a build machine
//! without X11 must still be able to run the workspace's tests.

#![cfg(target_os = "linux")]

use dyetrace_platform::traits::{GameWindow, Mouse, Screen};
use dyetrace_platform_linux::{X11Mouse, X11Screen, X11Window};

fn screen() -> Option<X11Screen> {
    let screen = X11Screen::new();
    let (w, h) = screen.screen_size();
    if w > 0 && h > 0 {
        Some(screen)
    } else {
        eprintln!("no X display; skipping backend test");
        None
    }
}

#[test]
fn screen_reports_its_size() {
    if let Some(screen) = screen() {
        let (w, h) = screen.screen_size();
        assert!(w > 0 && h > 0, "screen size {w}x{h}");
    }
}

#[test]
fn capture_returns_the_requested_rectangle() {
    if let Some(screen) = screen() {
        let image = screen.capture(0, 0, 64, 48).expect("capture failed");
        assert_eq!(image.width(), 64);
        assert_eq!(image.height(), 48);
        assert_eq!(image.as_slice().len(), 64 * 48);
    }
}

#[test]
fn capture_outside_the_screen_returns_nothing() {
    if let Some(screen) = screen() {
        let (w, h) = screen.screen_size();
        assert!(screen.capture(w, h, 64, 48).is_none());
    }
}

#[test]
fn pointer_position_is_on_screen() {
    if let Some(screen) = screen() {
        let (w, h) = screen.screen_size();
        let (x, y) = X11Mouse::new().position();
        assert!(x >= 0 && y >= 0 && x <= w && y <= h, "pointer at {x},{y}");
    }
}

#[test]
fn the_pointer_can_be_placed_where_it_already_is() {
    if screen().is_some() {
        let mouse = X11Mouse::new();
        let (x, y) = mouse.position();
        assert!(
            mouse.move_to(x, y),
            "move_to reported failure at the pointer's own position ({x},{y})"
        );
    }
}

#[test]
fn window_lookup_is_consistent() {
    if screen().is_some() {
        let window = X11Window::mabinogi();
        if let Some(id) = window.find() {
            let rect = window.rect(id).expect("rectangle for a live window");
            assert!(rect.w > 0 && rect.h > 0, "rect {rect:?}");
        }
    }
}

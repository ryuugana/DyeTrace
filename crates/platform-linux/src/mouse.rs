//! Pointer movement and clicking through the XTEST extension.
//!
//! XTEST injects the movement at the server, so the client receives a real
//! `MotionNotify` rather than a bare warp - which is what the game's hover
//! state needs.  Should a server not offer XTEST, movement falls back to
//! `WarpPointer` and clicks to synthetic button events, and [`move_to`] still
//! reports whether the pointer actually arrived.
//!
//! [`move_to`]: dyetrace_platform::traits::Mouse::move_to

use crate::connection::X11;
use dyetrace_platform::traits::Mouse;
use std::thread::sleep;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ButtonPressEvent, ButtonReleaseEvent, ConnectionExt, EventMask, KeyButMask, BUTTON_PRESS_EVENT,
    BUTTON_RELEASE_EVENT,
};
use x11rb::protocol::xtest;

/// Drives the real pointer.
pub struct X11Mouse {
    x11: Option<X11>,
    /// Delay between button down and up.
    pub press_delay: Duration,
}

impl Default for X11Mouse {
    fn default() -> Self {
        Self::new()
    }
}

impl X11Mouse {
    pub fn new() -> Self {
        Self {
            x11: X11::connect(),
            press_delay: Duration::from_millis(30),
        }
    }

    /// Whether a display was found.  Calls quietly do nothing without one.
    pub fn connected(&self) -> bool {
        self.x11.is_some()
    }

    /// The pointer's position, or `None` when there is no display to ask.
    fn at(&self) -> Option<(i32, i32)> {
        let x11 = self.x11.as_ref()?;
        let reply = x11.conn.query_pointer(x11.root).ok()?.reply().ok()?;
        Some((reply.root_x as i32, reply.root_y as i32))
    }
}

/// Ask XTEST to put the pointer at a point, without waiting.
fn fake_motion(x11: &X11, x: i32, y: i32) -> bool {
    xtest::fake_input(
        &x11.conn,
        x11rb::protocol::xproto::MOTION_NOTIFY_EVENT,
        0,
        0, // CurrentTime
        x11.root,
        x as i16,
        y as i16,
        0,
    )
    .is_ok()
}

/// Send a button event when XTEST is unavailable.  The event goes to the child
/// window under the pointer, with propagation on, so the client's own handler
/// sees it.
fn send_button(x11: &X11, press: bool, x: i32, y: i32) -> bool {
    let child = x11
        .conn
        .query_pointer(x11.root)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .map(|reply| reply.child)
        .filter(|child| *child != x11rb::NONE)
        .unwrap_or(x11.root);

    let response_type = if press {
        BUTTON_PRESS_EVENT
    } else {
        BUTTON_RELEASE_EVENT
    };

    let event = ButtonPressEvent {
        response_type,
        detail: 1,
        sequence: 0,
        time: 0,
        root: x11.root,
        event: child,
        child: x11rb::NONE,
        root_x: x as i16,
        root_y: y as i16,
        event_x: 0,
        event_y: 0,
        state: KeyButMask::default(),
        same_screen: true,
    };

    // ButtonReleaseEvent has the same layout, but a different type; build one
    // so the release is not a press event with a different response byte.
    let bytes: [u8; 32] = if press {
        event.into()
    } else {
        let release = ButtonReleaseEvent {
            response_type,
            detail: event.detail,
            sequence: event.sequence,
            time: event.time,
            root: event.root,
            event: event.event,
            child: event.child,
            root_x: event.root_x,
            root_y: event.root_y,
            event_x: event.event_x,
            event_y: event.event_y,
            state: event.state,
            same_screen: event.same_screen,
        };
        release.into()
    };

    let mask = EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE;
    let _ = x11.conn.send_event(true, child, mask, bytes);
    true
}

fn click_with(x11: &X11, press: bool, x: i32, y: i32) {
    let type_ = if press {
        x11rb::protocol::xproto::BUTTON_PRESS_EVENT
    } else {
        BUTTON_RELEASE_EVENT
    };
    if xtest::fake_input(&x11.conn, type_, 1, 0, x11.root, x as i16, y as i16, 0).is_err() {
        send_button(x11, press, x, y);
    }
}

impl Mouse for X11Mouse {
    fn position(&self) -> (i32, i32) {
        self.at().unwrap_or((0, 0))
    }

    /// Move the pointer and confirm it arrived.  Like the Win32 backend, this
    /// tries a few times and returns false rather than let a caller click
    /// somewhere else.
    fn move_to(&self, x: i32, y: i32) -> bool {
        let Some(x11) = self.x11.as_ref() else {
            return false;
        };

        for _ in 0..3 {
            let sent = fake_motion(x11, x, y);
            if !sent {
                let _ =
                    x11.conn
                        .warp_pointer(x11rb::NONE, x11.root, 0, 0, 0, 0, x as i16, y as i16);
            }
            let _ = x11.conn.flush();
            sleep(Duration::from_millis(10));

            let Some((px, py)) = self.at() else {
                continue;
            };
            if (px - x).abs() <= 1 && (py - y).abs() <= 1 {
                return true;
            }
        }

        false
    }

    fn click(&self) {
        let Some(x11) = self.x11.as_ref() else {
            return;
        };
        let (x, y) = self.position();
        click_with(x11, true, x, y);
        // Flush before the pause: queued back-to-back, the client would see a
        // press and release in the same instant.
        let _ = x11.conn.flush();
        sleep(self.press_delay);
        click_with(x11, false, x, y);
        let _ = x11.conn.flush();
    }

    /// Relative motion, used to make a client re-evaluate its hover state.
    fn nudge(&self, dx: i32, dy: i32) {
        let Some(x11) = self.x11.as_ref() else {
            return;
        };
        let Some((x, y)) = self.at() else {
            return;
        };
        let _ = fake_motion(x11, x + dx, y + dy);
        let _ = x11.conn.flush();
    }
}

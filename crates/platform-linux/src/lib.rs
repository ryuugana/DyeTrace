#![cfg(target_os = "linux")]
//! X11 backend: the calls the client needs, behind the three platform traits.
//!
//! * [`X11Screen`] - `GetImage` from the root window, or from the client's own
//!   drawable when the window has to be read while covered; a rootless
//!   XWayland root is rebuilt from the visible top-level windows
//! * [`X11Mouse`] - XTEST motion and clicks, then a read-back so a click is
//!   never sent to the wrong place
//! * [`X11Window`] - the window manager's client list, `WM_CLASS`, and EWMH
//!   activation
//! * [`KeyWatcher`] - `QueryKeymap`, which answers wherever the focus is
//!
//! The protocol is spoken by `x11rb`, a pure-Rust X11 client, so the backend
//! needs neither the libX11 development headers nor an unversioned shared
//! object to build.  It runs on an X11 server and, under Wayland, on the
//! XWayland server the game itself runs on.

mod connection;
mod keys;
mod mouse;
mod pixels;
mod screen;
mod window;

pub use keys::KeyWatcher;
pub use mouse::X11Mouse;
pub use screen::X11Screen;
pub use window::X11Window;

pub use dyetrace_platform::traits::{GameWindow, Mouse, Rect, Screen, WindowId};

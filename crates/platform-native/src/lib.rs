//! The backend for the operating system this build targets.
//!
//! Everything above this crate — the CLI, the engine, the window — names one
//! screen, one pointer, one window finder and one key watcher.  Keeping the
//! choice here means the rest of the program has no `cfg` branches of its own.

#[cfg(not(any(windows, target_os = "linux")))]
compile_error!("DyeTrace supports Windows and Linux; this target has no backend");

#[cfg(windows)]
mod imp {
    pub use dyetrace_platform_win::{
        KeyWatcher, WinMouse as NativeMouse, WinScreen as NativeScreen, WinWindow as NativeWindow,
    };
}

#[cfg(target_os = "linux")]
mod imp {
    pub use dyetrace_platform_linux::{
        KeyWatcher, X11Mouse as NativeMouse, X11Screen as NativeScreen, X11Window as NativeWindow,
    };
}

#[cfg(any(windows, target_os = "linux"))]
pub use imp::{KeyWatcher, NativeMouse, NativeScreen, NativeWindow};

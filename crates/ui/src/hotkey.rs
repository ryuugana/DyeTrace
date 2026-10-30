//! Watching for the stop key.
//!
//! While a cycle runs the client is in the foreground, so this window never sees
//! the keystroke.  The key is polled with `GetAsyncKeyState` instead, which
//! answers wherever the keystroke is going, and the poller raises the worker's
//! stop flag when it goes down.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use dyetrace_platform_win::key_down;
use eframe::egui;

/// How often the key is looked at.
const POLL: Duration = Duration::from_millis(60);
/// Space, until the window says otherwise.
pub const DEFAULT_KEY: i32 = 0x20;

/// A key that stops a running cycle, wherever the keyboard focus is.
pub struct StopKey {
    key: Arc<Mutex<i32>>,
}

impl StopKey {
    /// Start watching, raising `stop` when the key goes down.
    pub fn watch(stop: Arc<AtomicBool>) -> Self {
        let key = Arc::new(Mutex::new(DEFAULT_KEY));
        let watched = Arc::clone(&key);

        thread::spawn(move || loop {
            thread::sleep(POLL);

            let current = watched.lock().map(|slot| *slot).unwrap_or(0);
            if current == 0 || !key_down(current) {
                continue;
            }

            stop.store(true, Ordering::SeqCst);
            // A held key must not stop the next cycle as well, so wait for it to
            // come back up before looking again.
            while key_down(current) {
                thread::sleep(POLL);
            }
        });

        Self { key }
    }

    /// Change the key, by virtual-key code.
    pub fn set(&self, vk: i32) {
        if let Ok(mut slot) = self.key.lock() {
            *slot = vk;
        }
    }
}

/// The Windows virtual-key code for a key the window can name.
///
/// Only keys with a stable code are offered - letters, digits, the named
/// editing keys and the function keys - so the recorded key and the polled one
/// are always the same thing.
pub fn virtual_key(key: egui::Key) -> Option<i32> {
    use eframe::egui::Key;

    let code = match key {
        Key::A => 0x41,
        Key::B => 0x42,
        Key::C => 0x43,
        Key::D => 0x44,
        Key::E => 0x45,
        Key::F => 0x46,
        Key::G => 0x47,
        Key::H => 0x48,
        Key::I => 0x49,
        Key::J => 0x4A,
        Key::K => 0x4B,
        Key::L => 0x4C,
        Key::M => 0x4D,
        Key::N => 0x4E,
        Key::O => 0x4F,
        Key::P => 0x50,
        Key::Q => 0x51,
        Key::R => 0x52,
        Key::S => 0x53,
        Key::T => 0x54,
        Key::U => 0x55,
        Key::V => 0x56,
        Key::W => 0x57,
        Key::X => 0x58,
        Key::Y => 0x59,
        Key::Z => 0x5A,

        Key::Num0 => 0x30,
        Key::Num1 => 0x31,
        Key::Num2 => 0x32,
        Key::Num3 => 0x33,
        Key::Num4 => 0x34,
        Key::Num5 => 0x35,
        Key::Num6 => 0x36,
        Key::Num7 => 0x37,
        Key::Num8 => 0x38,
        Key::Num9 => 0x39,

        Key::F1 => 0x70,
        Key::F2 => 0x71,
        Key::F3 => 0x72,
        Key::F4 => 0x73,
        Key::F5 => 0x74,
        Key::F6 => 0x75,
        Key::F7 => 0x76,
        Key::F8 => 0x77,
        Key::F9 => 0x78,
        Key::F10 => 0x79,
        Key::F11 => 0x7A,
        Key::F12 => 0x7B,

        Key::Space => 0x20,
        Key::Enter => 0x0D,
        Key::Tab => 0x09,
        Key::Escape => 0x1B,
        Key::Backspace => 0x08,
        Key::Insert => 0x2D,
        Key::Delete => 0x2E,
        Key::Home => 0x24,
        Key::End => 0x23,
        Key::PageUp => 0x21,
        Key::PageDown => 0x22,
        Key::ArrowLeft => 0x25,
        Key::ArrowUp => 0x26,
        Key::ArrowRight => 0x27,
        Key::ArrowDown => 0x28,

        Key::Minus => 0xBD,
        Key::Plus | Key::Equals => 0xBB,
        Key::Comma => 0xBC,
        Key::Period => 0xBE,
        Key::Slash => 0xBF,
        Key::Semicolon => 0xBA,
        Key::Quote => 0xDE,
        Key::OpenBracket => 0xDB,
        Key::CloseBracket => 0xDD,
        Key::Backslash => 0xDC,
        Key::Backtick => 0xC0,

        _ => return None,
    };

    Some(code)
}

/// The virtual-key code of a key named by the window, for the default.
pub fn default_key() -> egui::Key {
    egui::Key::Space
}

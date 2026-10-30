//! Watching for the stop key.
//!
//! While a cycle runs the client is in the foreground, so this window never sees
//! the keystroke.  The key is polled through the platform's global key state
//! instead (`GetAsyncKeyState` on Windows, `QueryKeymap` on X11), which answers
//! wherever the keystroke is going, and the poller raises the worker's stop flag
//! when it goes down.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use dyetrace_platform_native::KeyWatcher;
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
        let keys = KeyWatcher::new();

        thread::spawn(move || loop {
            thread::sleep(POLL);

            let current = watched.lock().map(|slot| *slot).unwrap_or(0);
            if current == 0 || !keys.is_down(current) {
                continue;
            }

            stop.store(true, Ordering::SeqCst);
            // A held key must not stop the next cycle as well, so wait for it to
            // come back up before looking again.
            while keys.is_down(current) {
                thread::sleep(POLL);
            }
        });

        Self { key }
    }

    /// Change the key, by key id.
    pub fn set(&self, vk: i32) {
        if let Ok(mut slot) = self.key.lock() {
            *slot = vk;
        }
    }
}

/// Every key the window can offer as a stop key, with its key id.
///
/// The id is the Win32 virtual-key number, which the config file has always
/// stored; the X11 backend maps it to the matching keysym.  Only keys with a
/// stable id are listed - letters, digits, the named editing keys and the
/// function keys - so a recorded key and the polled one are always the same
/// thing.
const KEY_CODES: &[(egui::Key, i32)] = &[
    (egui::Key::A, 0x41),
    (egui::Key::B, 0x42),
    (egui::Key::C, 0x43),
    (egui::Key::D, 0x44),
    (egui::Key::E, 0x45),
    (egui::Key::F, 0x46),
    (egui::Key::G, 0x47),
    (egui::Key::H, 0x48),
    (egui::Key::I, 0x49),
    (egui::Key::J, 0x4A),
    (egui::Key::K, 0x4B),
    (egui::Key::L, 0x4C),
    (egui::Key::M, 0x4D),
    (egui::Key::N, 0x4E),
    (egui::Key::O, 0x4F),
    (egui::Key::P, 0x50),
    (egui::Key::Q, 0x51),
    (egui::Key::R, 0x52),
    (egui::Key::S, 0x53),
    (egui::Key::T, 0x54),
    (egui::Key::U, 0x55),
    (egui::Key::V, 0x56),
    (egui::Key::W, 0x57),
    (egui::Key::X, 0x58),
    (egui::Key::Y, 0x59),
    (egui::Key::Z, 0x5A),
    (egui::Key::Num0, 0x30),
    (egui::Key::Num1, 0x31),
    (egui::Key::Num2, 0x32),
    (egui::Key::Num3, 0x33),
    (egui::Key::Num4, 0x34),
    (egui::Key::Num5, 0x35),
    (egui::Key::Num6, 0x36),
    (egui::Key::Num7, 0x37),
    (egui::Key::Num8, 0x38),
    (egui::Key::Num9, 0x39),
    (egui::Key::F1, 0x70),
    (egui::Key::F2, 0x71),
    (egui::Key::F3, 0x72),
    (egui::Key::F4, 0x73),
    (egui::Key::F5, 0x74),
    (egui::Key::F6, 0x75),
    (egui::Key::F7, 0x76),
    (egui::Key::F8, 0x77),
    (egui::Key::F9, 0x78),
    (egui::Key::F10, 0x79),
    (egui::Key::F11, 0x7A),
    (egui::Key::F12, 0x7B),
    (egui::Key::Space, 0x20),
    (egui::Key::Enter, 0x0D),
    (egui::Key::Tab, 0x09),
    (egui::Key::Escape, 0x1B),
    (egui::Key::Backspace, 0x08),
    (egui::Key::Insert, 0x2D),
    (egui::Key::Delete, 0x2E),
    (egui::Key::Home, 0x24),
    (egui::Key::End, 0x23),
    (egui::Key::PageUp, 0x21),
    (egui::Key::PageDown, 0x22),
    (egui::Key::ArrowLeft, 0x25),
    (egui::Key::ArrowUp, 0x26),
    (egui::Key::ArrowRight, 0x27),
    (egui::Key::ArrowDown, 0x28),
    (egui::Key::Minus, 0xBD),
    (egui::Key::Plus, 0xBB),
    (egui::Key::Equals, 0xBB),
    (egui::Key::Comma, 0xBC),
    (egui::Key::Period, 0xBE),
    (egui::Key::Slash, 0xBF),
    (egui::Key::Semicolon, 0xBA),
    (egui::Key::Quote, 0xDE),
    (egui::Key::OpenBracket, 0xDB),
    (egui::Key::CloseBracket, 0xDD),
    (egui::Key::Backslash, 0xDC),
    (egui::Key::Backtick, 0xC0),
];

/// The key id of a key the window can name.
pub fn virtual_key(key: egui::Key) -> Option<i32> {
    KEY_CODES
        .iter()
        .find(|(known, _)| *known == key)
        .map(|(_, code)| *code)
}

/// The key for a code recorded earlier, if it is one we can still name.
pub fn key_for(code: i32) -> Option<egui::Key> {
    KEY_CODES
        .iter()
        .find(|(_, known)| *known == code)
        .map(|(key, _)| *key)
}

/// The default stop key, by name.
pub fn default_key() -> egui::Key {
    egui::Key::Space
}

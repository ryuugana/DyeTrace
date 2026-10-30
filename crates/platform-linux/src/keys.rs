//! Watching the stop key with `QueryKeymap`.
//!
//! `QueryKeymap` reports the physical keys held down, wherever the keyboard
//! focus is, which is what the stop key needs while the game has focus.  The
//! key is recorded as the same virtual-key number the Win32 backend uses; this
//! module translates it to an X11 keysym and then to the keycode the server
//! actually reports.

use std::sync::Mutex;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, Keycode};
use x11rb::rust_connection::RustConnection;

use crate::connection::X11;

/// Watches one key at a time, by virtual-key number.
pub struct KeyWatcher {
    state: Option<Mutex<KeyState>>,
}

struct KeyState {
    conn: RustConnection,
    map: KeyMap,
}

/// The server's keysym-to-keycode table, kept apart from the connection so it
/// can be checked without a display.
struct KeyMap {
    min_keycode: Keycode,
    keysyms_per_keycode: usize,
    keysyms: Vec<u32>,
}

impl KeyState {
    fn connect() -> Option<Self> {
        let x11 = X11::connect()?;
        let setup = x11.conn.setup();
        let min_keycode = setup.min_keycode;
        let max_keycode = setup.max_keycode;
        let count = max_keycode.checked_sub(min_keycode)?.checked_add(1)?;

        let reply = x11
            .conn
            .get_keyboard_mapping(min_keycode, count)
            .ok()?
            .reply()
            .ok()?;

        Some(Self {
            conn: x11.conn,
            map: KeyMap {
                min_keycode,
                keysyms_per_keycode: reply.keysyms_per_keycode as usize,
                keysyms: reply.keysyms,
            },
        })
    }

    fn is_down(&self, keysym: u32) -> bool {
        let Some(keycode) = self.map.keycode(keysym) else {
            return false;
        };
        let Some(reply) = self
            .conn
            .query_keymap()
            .ok()
            .and_then(|cookie| cookie.reply().ok())
        else {
            return false;
        };
        let byte = (keycode / 8) as usize;
        let bit = keycode % 8;
        reply
            .keys
            .get(byte)
            .map(|value| value & (1 << bit) != 0)
            .unwrap_or(false)
    }
}

impl KeyMap {
    fn keycode(&self, keysym: u32) -> Option<Keycode> {
        if self.keysyms_per_keycode == 0 {
            return None;
        }
        for (index, levels) in self.keysyms.chunks(self.keysyms_per_keycode).enumerate() {
            if levels.contains(&keysym) {
                return Some(self.min_keycode.wrapping_add(index as u8));
            }
        }
        None
    }
}

impl Default for KeyWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyWatcher {
    pub fn new() -> Self {
        Self {
            state: KeyState::connect().map(Mutex::new),
        }
    }

    /// Whether a display was found.  Without one, no key is ever down.
    pub fn connected(&self) -> bool {
        self.state.is_some()
    }

    /// Is the key with this virtual-key number held down?
    pub fn is_down(&self, key: i32) -> bool {
        let Some(keysym) = vk_to_keysym(key) else {
            return false;
        };
        let Some(state) = &self.state else {
            return false;
        };
        match state.lock() {
            Ok(state) => state.is_down(keysym),
            Err(_) => false,
        }
    }
}

/// Translate the Win32 virtual-key numbering the interface and config file use
/// into an X11 keysym.
///
/// Letters become their lowercase keysym because that is the first level of a
/// normal keymap; every level is searched, so a remapped key still matches.
fn vk_to_keysym(vk: i32) -> Option<u32> {
    let vk = vk as u32;
    let keysym = match vk {
        0x30..=0x39 => vk,                   // 0-9
        0x41..=0x5A => 0x61 + (vk - 0x41),   // A-Z -> a-z
        0x70..=0x7B => 0xFFBE + (vk - 0x70), // F1-F12
        0x20 => 0x0020,                      // space
        0x0D => 0xFF0D,                      // return
        0x09 => 0xFF09,                      // tab
        0x1B => 0xFF1B,                      // escape
        0x08 => 0xFF08,                      // backspace
        0x2D => 0xFF63,                      // insert
        0x2E => 0xFFFF,                      // delete
        0x24 => 0xFF50,                      // home
        0x23 => 0xFF57,                      // end
        0x21 => 0xFF55,                      // page up
        0x22 => 0xFF56,                      // page down
        0x25 => 0xFF51,                      // left
        0x26 => 0xFF52,                      // up
        0x27 => 0xFF53,                      // right
        0x28 => 0xFF54,                      // down
        0xBD => 0x002D,                      // minus
        0xBB => 0x003D,                      // equals / plus
        0xBC => 0x002C,                      // comma
        0xBE => 0x002E,                      // period
        0xBF => 0x002F,                      // slash
        0xBA => 0x003B,                      // semicolon
        0xDE => 0x0027,                      // apostrophe
        0xDB => 0x005B,                      // open bracket
        0xDD => 0x005D,                      // close bracket
        0xDC => 0x005C,                      // backslash
        0xC0 => 0x0060,                      // backtick
        _ => return None,
    };
    Some(keysym)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_digits_map_to_ascii_keysyms() {
        assert_eq!(vk_to_keysym(0x41), Some(0x61)); // A -> a
        assert_eq!(vk_to_keysym(0x5A), Some(0x7A)); // Z -> z
        assert_eq!(vk_to_keysym(0x30), Some(0x30)); // 0
        assert_eq!(vk_to_keysym(0x39), Some(0x39)); // 9
    }

    #[test]
    fn function_and_named_keys_use_the_x11_range() {
        assert_eq!(vk_to_keysym(0x70), Some(0xFFBE)); // F1
        assert_eq!(vk_to_keysym(0x7B), Some(0xFFC9)); // F12
        assert_eq!(vk_to_keysym(0x20), Some(0x0020)); // space
        assert_eq!(vk_to_keysym(0x1B), Some(0xFF1B)); // escape
        assert_eq!(vk_to_keysym(0x25), Some(0xFF51)); // left
    }

    #[test]
    fn an_unknown_key_is_ignored() {
        assert_eq!(vk_to_keysym(0x07), None);
        assert_eq!(vk_to_keysym(-1), None);
    }

    #[test]
    fn keycodes_are_found_at_any_level() {
        // One keycode with two levels: lowercase 'a', then 'A'.
        let map = KeyMap {
            min_keycode: 38,
            keysyms_per_keycode: 2,
            keysyms: vec![0x61, 0x41, 0xFF0D, 0xFF0D],
        };
        assert_eq!(map.keycode(0x61), Some(38));
        assert_eq!(map.keycode(0x41), Some(38));
        assert_eq!(map.keycode(0xFF0D), Some(39));
        assert_eq!(map.keycode(0x62), None);
    }
}

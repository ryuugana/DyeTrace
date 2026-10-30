//! Finding and raising the client window.
//!
//! The client is a normal X11 toplevel, so it is found through the window
//! manager's client list (falling back to a walk of the window tree) and
//! identified by `WM_CLASS` or its title.  Raising it uses the EWMH
//! `_NET_ACTIVE_WINDOW` message, which is the X11 equivalent of asking the
//! window manager to bring a window to the front; `SetInputFocus` is the
//! fallback for a server running without a window manager.

use crate::connection::X11;
use dyetrace_platform::traits::{GameWindow, Rect, WindowId};
use std::thread::sleep;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt, EventMask, InputFocus,
    MapState, StackMode, Window,
};

struct Atoms {
    net_client_list: Atom,
    net_wm_name: Atom,
    net_active_window: Atom,
    utf8_string: Atom,
}

impl Atoms {
    fn new(x11: &X11) -> Option<Self> {
        Some(Self {
            net_client_list: intern(x11, b"_NET_CLIENT_LIST")?,
            net_wm_name: intern(x11, b"_NET_WM_NAME")?,
            net_active_window: intern(x11, b"_NET_ACTIVE_WINDOW")?,
            utf8_string: intern(x11, b"UTF8_STRING")?,
        })
    }
}

fn intern(x11: &X11, name: &[u8]) -> Option<Atom> {
    let reply = x11.conn.intern_atom(true, name).ok()?.reply().ok()?;
    (reply.atom != x11rb::NONE).then_some(reply.atom)
}

/// Locates the client by window class and/or title.
pub struct X11Window {
    x11: Option<X11>,
    class: Option<String>,
    title: Option<String>,
    atoms: Option<Atoms>,
    /// How long to wait after raising the client before reading the screen.
    pub focus_delay: Duration,
}

impl X11Window {
    /// The client registers a window whose class, and usually whose title,
    /// says "Mabinogi".  Either is enough, because Wine does not always pass
    /// the Windows class through unchanged.
    pub fn mabinogi() -> Self {
        Self::new(Some("Mabinogi".to_string()), Some("Mabinogi".to_string()))
    }

    pub fn with_class(class: &str) -> Self {
        Self::new(Some(class.to_string()), None)
    }

    fn new(class: Option<String>, title: Option<String>) -> Self {
        let x11 = X11::connect();
        let atoms = x11.as_ref().and_then(Atoms::new);
        Self {
            x11,
            class,
            title,
            atoms,
            focus_delay: Duration::from_millis(250),
        }
    }

    /// Whether a display was found.  Calls quietly do nothing without one.
    pub fn connected(&self) -> bool {
        self.x11.is_some()
    }

    /// Windows worth looking at, and whether they came from the window
    /// manager's client list.
    ///
    /// The client list is cheap and correct on a normal desktop.  It is not
    /// always complete, though: Wine's virtual desktop can own the toplevel
    /// while the client is a child, so a miss sends the caller to a tree walk.
    fn candidates(&self, x11: &X11) -> (Vec<Window>, bool) {
        if let Some(atoms) = &self.atoms {
            if let Some(list) = self.client_list(x11, atoms) {
                if !list.is_empty() {
                    return (list, true);
                }
            }
        }

        let mut found = Vec::new();
        walk(x11, x11.root, 0, &mut found);
        (found, false)
    }

    /// Search a list of windows for the client.
    ///
    /// A visible class match is what the caller wants.  Title matches come
    /// second so a browser showing a page about Mabinogi does not beat the
    /// client itself; unmapped windows come last, so a minimised client is
    /// still reported (the capture will fail, which the caller handles).
    fn search(&self, x11: &X11, candidates: &[Window]) -> Option<WindowId> {
        let checks: [fn(&Self, &X11, Window) -> bool; 2] =
            [Self::matches_class, Self::matches_title];
        for visible_only in [true, false] {
            for check in checks {
                for &window in candidates {
                    if visible_only && !self.viewable(x11, window) {
                        continue;
                    }
                    if check(self, x11, window) {
                        return Some(WindowId(window as usize));
                    }
                }
            }
        }
        None
    }

    fn client_list(&self, x11: &X11, atoms: &Atoms) -> Option<Vec<Window>> {
        let reply = x11
            .conn
            .get_property(
                false,
                x11.root,
                atoms.net_client_list,
                AtomEnum::WINDOW,
                0,
                4096,
            )
            .ok()?
            .reply()
            .ok()?;
        if reply.format != 32 {
            return None;
        }
        Some(
            reply
                .value
                .chunks(4)
                .filter_map(|chunk| <[u8; 4]>::try_from(chunk).ok())
                .map(u32::from_ne_bytes)
                .collect(),
        )
    }

    /// Does the window's `WM_CLASS` name the class we were given?
    ///
    /// This is the strong match: it is what Wine passes through from the
    /// client, and it is far less likely to belong to a browser tab than a
    /// title is.
    fn matches_class(&self, x11: &X11, window: Window) -> bool {
        let Some(class) = &self.class else {
            return false;
        };
        let Some(prop) = property(x11, window, AtomEnum::WM_CLASS, AtomEnum::STRING) else {
            return false;
        };
        let wanted = class.as_bytes();
        prop.split(|byte| *byte == 0)
            .any(|part| part.eq_ignore_ascii_case(wanted))
    }

    /// Does the window's title contain the title we were given?
    fn matches_title(&self, x11: &X11, window: Window) -> bool {
        let Some(title) = &self.title else {
            return false;
        };
        self.window_title(x11, window)
            .map(|name| name.to_lowercase().contains(&title.to_lowercase()))
            .unwrap_or(false)
    }

    fn window_title(&self, x11: &X11, window: Window) -> Option<String> {
        if let Some(atoms) = &self.atoms {
            if let Some(bytes) = property(x11, window, atoms.net_wm_name, atoms.utf8_string) {
                if !bytes.is_empty() {
                    return Some(String::from_utf8_lossy(&bytes).into_owned());
                }
            }
        }
        property(x11, window, AtomEnum::WM_NAME, AtomEnum::ANY)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    }

    fn viewable(&self, x11: &X11, window: Window) -> bool {
        x11.conn
            .get_window_attributes(window)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map(|attributes| attributes.map_state == MapState::VIEWABLE)
            .unwrap_or(false)
    }

    fn focused(&self, x11: &X11, window: Window) -> bool {
        let Some(reply) = x11
            .conn
            .get_input_focus()
            .ok()
            .and_then(|cookie| cookie.reply().ok())
        else {
            return false;
        };
        if reply.focus == window {
            return true;
        }
        // A toolkit may put the input focus on a child of the toplevel.
        x11.conn
            .query_tree(window)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map(|tree| tree.children.contains(&reply.focus))
            .unwrap_or(false)
    }

    fn activate(&self, x11: &X11, window: Window) -> bool {
        let Some(atoms) = &self.atoms else {
            return false;
        };
        let event = ClientMessageEvent::new(
            32,
            window,
            atoms.net_active_window,
            // source indication 2: a pager, which window managers do not
            // subject to focus-stealing prevention.
            [2u32, 0, 0, 0, 0],
        );
        let _ = x11.conn.send_event(
            false,
            x11.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        );
        true
    }

    fn raise(&self, x11: &X11, window: Window) {
        let _ = x11.conn.configure_window(
            window,
            &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
        );
    }

    fn take_focus(&self, x11: &X11, window: Window) {
        let _ = x11.conn.set_input_focus(InputFocus::PARENT, window, 0u32);
        let _ = x11.conn.flush();
    }
}

impl GameWindow for X11Window {
    fn find(&self) -> Option<WindowId> {
        let x11 = self.x11.as_ref()?;
        let (candidates, from_client_list) = self.candidates(x11);

        if let Some(id) = self.search(x11, &candidates) {
            return Some(id);
        }

        // A client list that did not contain the game may still have the game
        // nested inside one of its entries.
        if from_client_list {
            let mut walked = Vec::new();
            walk(x11, x11.root, 0, &mut walked);
            return self.search(x11, &walked);
        }

        None
    }

    fn rect(&self, id: WindowId) -> Option<Rect> {
        let x11 = self.x11.as_ref()?;
        let window = id.0 as Window;
        let geometry = x11.conn.get_geometry(window).ok()?.reply().ok()?;
        let translated = x11
            .conn
            .translate_coordinates(window, x11.root, 0, 0)
            .ok()?
            .reply()
            .ok()?;
        if !translated.same_screen {
            return None;
        }
        Some(Rect {
            x: translated.dst_x as i32,
            y: translated.dst_y as i32,
            w: geometry.width as i32,
            h: geometry.height as i32,
        })
    }

    /// Raise and focus the client so clicks sent at screen coordinates reach
    /// it.  Focus is confirmed, never assumed.
    fn focus(&self, id: WindowId) -> bool {
        let Some(x11) = self.x11.as_ref() else {
            return false;
        };
        let window = id.0 as Window;

        if self.focused(x11, window) {
            return true;
        }

        let _ = self.activate(x11, window);
        self.raise(x11, window);
        self.take_focus(x11, window);
        sleep(Duration::from_millis(80));

        if self.focused(x11, window) {
            sleep(self.focus_delay);
            return true;
        }

        // Without a window manager (or when it ignored the activation), the
        // direct request is all there is; try it once more before giving up.
        self.raise(x11, window);
        self.take_focus(x11, window);
        sleep(self.focus_delay);
        self.focused(x11, window)
    }
}

fn property<P, T>(x11: &X11, window: Window, property: P, type_: T) -> Option<Vec<u8>>
where
    P: Into<Atom>,
    T: Into<Atom>,
{
    let reply = x11
        .conn
        .get_property(false, window, property, type_, 0, 4096)
        .ok()?
        .reply()
        .ok()?;
    if reply.format == 0 || reply.value.is_empty() {
        None
    } else {
        Some(reply.value)
    }
}

/// Depth-first walk of the window tree, for servers without a client list.
fn walk(x11: &X11, window: Window, depth: usize, out: &mut Vec<Window>) {
    if depth > 8 || out.len() > 4096 {
        return;
    }
    let Some(tree) = x11
        .conn
        .query_tree(window)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
    else {
        return;
    };
    for child in tree.children {
        out.push(child);
        walk(x11, child, depth + 1, out);
    }
}

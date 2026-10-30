//! Reading the client's dye button.
//!
//! The button in the dye window has two faces.  While the window is idle it
//! reads "Pick Color" in white; once a colour has been chosen it reads "Start
//! Dyeing" in grey, and pressing that applies the dye.  Cancel only discards a
//! chosen colour - pressed while the label still says "Pick Color" it closes the
//! dye window.
//!
//! The label is therefore the only safe way to decide what may be pressed.  The
//! two faces differ sharply: the white label has about forty pixels brighter
//! than luminance 130 and the grey one none at all, so a few bright pixels are
//! enough to tell them apart and anything in between is reported as unknown
//! rather than guessed at.

use crate::image::Image;

/// The label's box, relative to the palette's top-left corner.
pub const LABEL_X: i32 = 139;
pub const LABEL_Y: i32 = 383;
pub const LABEL_W: i32 = 60;
pub const LABEL_H: i32 = 28;

/// Luminance a glyph must reach to count as white text.
const BRIGHT: i32 = 130;
/// Bright pixels that only the white label has.
const BRIGHT_MIN: usize = 8;
/// Bright pixels the grey label can still show, from anti-aliasing.
const BRIGHT_MAX: usize = 2;

/// What the dye button is offering to do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Label {
    /// White "Pick Color": pressing it starts a pick.
    PickColor,
    /// Grey "Start Dyeing": a colour is chosen, so only Cancel may be pressed.
    StartDyeing,
    /// The label could not be read; nothing may be pressed.
    Unknown,
}

impl Label {
    pub fn name(self) -> &'static str {
        match self {
            Label::PickColor => "Pick Color",
            Label::StartDyeing => "Start Dyeing",
            Label::Unknown => "unreadable",
        }
    }
}

/// Read the button label from a whole-window grab.
pub fn read(frame: &Image, origin: (i32, i32)) -> Label {
    let x0 = origin.0 + LABEL_X;
    let y0 = origin.1 + LABEL_Y;
    if x0 < 0
        || y0 < 0
        || x0 + LABEL_W > frame.width() as i32
        || y0 + LABEL_H > frame.height() as i32
    {
        return Label::Unknown;
    }

    let mut bright = 0;
    for y in y0..y0 + LABEL_H {
        for x in x0..x0 + LABEL_W {
            if frame.at(x, y).luminance() > BRIGHT {
                bright += 1;
            }
        }
    }

    if bright >= BRIGHT_MIN {
        Label::PickColor
    } else if bright <= BRIGHT_MAX {
        Label::StartDyeing
    } else {
        Label::Unknown
    }
}

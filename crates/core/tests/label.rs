//! Telling the dye button's two faces apart is what keeps clicks safe, so the
//! reader is pinned: white text is "Pick Color" and grey text is "Start Dyeing",
//! and anything unclear must come back as unknown rather than a guess.

use dyetrace_core::image::{Image, Rgb};
use dyetrace_core::label::{self, Label};

const W: usize = 320;
const H: usize = 480;
const ORIGIN: (i32, i32) = (20, 30);

fn frame() -> Image {
    Image::filled(W, H, Rgb::new(0x24, 0x24, 0x24))
}

/// Draw `count` glyph pixels of the given brightness inside the label box.
fn glyphs(img: &mut Image, count: usize, brightness: u8) {
    let x0 = ORIGIN.0 + label::LABEL_X;
    let y0 = ORIGIN.1 + label::LABEL_Y;
    for i in 0..count {
        img.put_i(x0 + (i as i32 % 20), y0 + 10 + (i as i32 / 20), Rgb::new(brightness, brightness, brightness));
    }
}

#[test]
fn white_text_reads_as_pick_color() {
    let mut img = frame();
    glyphs(&mut img, 40, 0xF0);
    assert_eq!(label::read(&img, ORIGIN), Label::PickColor);
}

#[test]
fn grey_text_reads_as_start_dyeing() {
    let mut img = frame();
    glyphs(&mut img, 40, 0x70);
    assert_eq!(label::read(&img, ORIGIN), Label::StartDyeing);
}

#[test]
fn a_dark_button_reads_as_start_dyeing() {
    // An unlit label cannot be the white one, and refusing to press is the
    // safe reading of it.
    assert_eq!(label::read(&frame(), ORIGIN), Label::StartDyeing);
}

#[test]
fn an_ambiguous_label_is_unknown() {
    let mut img = frame();
    glyphs(&mut img, 5, 0xF0);
    assert_eq!(label::read(&img, ORIGIN), Label::Unknown);
}

#[test]
fn a_frame_that_does_not_cover_the_button_is_unknown() {
    let img = Image::filled(200, 200, Rgb::new(0x24, 0x24, 0x24));
    assert_eq!(label::read(&img, ORIGIN), Label::Unknown);
}

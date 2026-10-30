//! Telling one roll of the dye gradient from another.
//!
//! Every dye attempt draws a new random palette, so the sample offsets measured
//! on the last one are worthless on the next.  A signature is a coarse average
//! of the picture: it survives small rendering differences between two grabs of
//! the same palette and changes completely when the palette is rolled.

use dyetrace_core::image::{self, Image, Rgb};

fn gradient(seed: u8) -> Image {
    let mut img = Image::filled(256, 256, Rgb::black());
    for y in 0..256usize {
        for x in 0..256usize {
            img.put(
                x,
                y,
                Rgb::new(
                    (x as u8).wrapping_mul(3).wrapping_add(seed),
                    (y as u8).wrapping_mul(3),
                    0x80,
                ),
            );
        }
    }
    img
}

#[test]
fn the_same_picture_has_the_same_signature() {
    let one = gradient(0);
    let two = gradient(0);
    assert!(image::same_signature(&one.signature(), &two.signature(), 0));
}

#[test]
fn a_new_roll_has_a_different_signature() {
    let one = gradient(0).signature();
    let two = gradient(37).signature();
    assert!(!image::same_signature(&one, &two, 8));
}

#[test]
fn a_little_noise_is_tolerated() {
    // A few stray pixels - a cursor drawn over the edge of the palette, a
    // compression artefact - must not look like a new roll.
    let clean = gradient(0);
    let mut noisy = clean.clone();
    for i in 0..16usize {
        noisy.put(i * 15, i * 7, Rgb::new(250, 250, 250));
    }

    assert!(image::same_signature(&clean.signature(), &noisy.signature(), 8));
}

#[test]
fn different_pictures_are_never_confused() {
    let flat = Image::filled(256, 256, Rgb::new(0x20, 0x20, 0x20));
    assert!(!image::same_signature(
        &gradient(0).signature(),
        &flat.signature(),
        8
    ));
}

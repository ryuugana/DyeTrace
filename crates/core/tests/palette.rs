//! Telling the dye palette from the client's own panels.
//!
//! A menu or an inventory window is also a square with edges, and the square
//! detector used to take the strongest one it found - which, with the bags open,
//! was sometimes a bag.  The dye form draws the gradient inside a one-pixel black
//! line, and these tests pin that down as the difference.

use dyetrace_core::image::{Image, Rgb};
use dyetrace_core::palette;
use dyetrace_core::{CAP_H, CAP_W};

const W: usize = 700;
const H: usize = 420;

fn frame() -> Image {
    Image::filled(W, H, Rgb::new(0x30, 0x30, 0x30))
}

/// A gradient at `(x, y)` inside the black line the dye form draws: the
/// gradient is 254 px and the line is the 256 px square around it.
fn draw_palette(img: &mut Image, x: i32, y: i32) {
    for row in 0..CAP_H as i32 - 2 {
        for col in 0..CAP_W as i32 - 2 {
            img.put_i(
                x + col,
                y + row,
                Rgb::new((col * 3) as u8, (row * 3) as u8, 0x80),
            );
        }
    }
    for i in -1..CAP_W as i32 - 1 {
        for (px, py) in [
            (x + i, y - 1),
            (x + i, y + CAP_H as i32 - 2),
            (x - 1, y + i),
            (x + CAP_W as i32 - 2, y + i),
        ] {
            img.put_i(px, py, Rgb::black());
        }
    }
}

/// A panel of the interface: a bordered square with a grid of colourful icons
/// on a dark background, which scores well as a square.
fn draw_icon_panel(img: &mut Image, x: i32, y: i32) {
    for row in 0..CAP_H as i32 {
        for col in 0..CAP_W as i32 {
            img.put_i(x + col, y + row, Rgb::new(0x12, 0x12, 0x14));
        }
    }

    // A two-pixel chrome border, bright enough to be a strong edge on all four
    // sides.
    for i in 0..CAP_W as i32 {
        for d in 0..2 {
            let chrome = Rgb::new(0x9A, 0x86, 0x50);
            img.put_i(x + i, y + d, chrome);
            img.put_i(x + i, y + CAP_H as i32 - 1 - d, chrome);
            img.put_i(x + d, y + i, chrome);
            img.put_i(x + CAP_W as i32 - 1 - d, y + i, chrome);
        }
    }

    // Icons: 20 px squares every 26 px, in the dye-like colours an inventory
    // holds.
    for row in (10..CAP_H as i32 - 30).step_by(26) {
        for col in (10..CAP_W as i32 - 30).step_by(26) {
            let colour = Rgb::new(
                (col as u8).wrapping_mul(7),
                (row as u8).wrapping_mul(5),
                (col as u8).wrapping_mul(3) ^ (row as u8),
            );
            for j in 0..20 {
                for i in 0..20 {
                    img.put_i(x + col + i, y + row + j, colour);
                }
            }
        }
    }
}

#[test]
fn finds_the_gradient_inside_its_outline() {
    let mut img = frame();
    draw_palette(&mut img, 30, 40);

    assert_eq!(palette::find_square_field(&img), Some((30, 40)));
    assert!(palette::has_frame(&img, (30, 40)));
}

#[test]
fn a_panel_of_icons_is_not_the_palette() {
    let mut img = frame();
    draw_icon_panel(&mut img, 60, 80);

    // The panel is a square with four strong edges, so the old detector took it;
    // without the line around the gradient it cannot be the palette.
    assert_eq!(palette::find_square_field(&img), None);
    assert!(!palette::has_frame(&img, (60, 80)));
}

#[test]
fn the_palette_wins_over_a_panel_beside_it() {
    let mut img = frame();
    draw_palette(&mut img, 30, 40);
    draw_icon_panel(&mut img, 420, 90);

    assert_eq!(palette::find_square_field(&img), Some((30, 40)));
}

#[test]
fn a_gradient_without_the_line_is_not_the_palette() {
    // The line is what is being relied on: a bare square, however good a square
    // it is, is not enough.
    let mut img = frame();
    for row in 0..CAP_H as i32 {
        for col in 0..CAP_W as i32 {
            img.put_i(
                30 + col,
                40 + row,
                Rgb::new((col * 3) as u8, (row * 3) as u8, 0x80),
            );
        }
    }

    assert_eq!(palette::find_square_field(&img), None);
    assert!(!palette::has_frame(&img, (30, 40)));
}

#[test]
fn a_line_broken_along_one_side_still_counts() {
    // One stray pixel on the line is rendering noise, not a different panel.
    let mut img = frame();
    draw_palette(&mut img, 30, 40);
    img.put_i(30 + 100, 39, Rgb::new(0x30, 0x30, 0x30));

    assert!(palette::has_frame(&img, (30, 40)));
}

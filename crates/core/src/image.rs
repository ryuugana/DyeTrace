//! Pixel buffer and colour type shared by every stage.
//!
//! Pixels are stored as `0x00RRGGBB`, the layout the DIB section in the C
//! version holds, so captures can be handed over without conversion.

/// A colour, or a pixel.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn black() -> Self {
        Self { r: 0, g: 0, b: 0 }
    }

    pub const fn from_u32(v: u32) -> Self {
        Self {
            r: ((v >> 16) & 0xFF) as u8,
            g: ((v >> 8) & 0xFF) as u8,
            b: (v & 0xFF) as u8,
        }
    }

    pub const fn to_u32(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | self.b as u32
    }

    /// Rec.601-ish luminance, weighted towards green the way the eye is.
    pub const fn luminance(self) -> i32 {
        ((self.r as i32) * 77 + (self.g as i32) * 151 + (self.b as i32) * 28) >> 8
    }

    pub const fn max_channel(self) -> u8 {
        if self.r > self.g {
            if self.r > self.b { self.r } else { self.b }
        } else if self.g > self.b {
            self.g
        } else {
            self.b
        }
    }

    pub const fn min_channel(self) -> u8 {
        if self.r < self.g {
            if self.r < self.b { self.r } else { self.b }
        } else if self.g < self.b {
            self.g
        } else {
            self.b
        }
    }

    /// Saturated enough to be part of a colour gradient rather than chrome.
    pub const fn is_colourful(self) -> bool {
        self.max_channel() > 60 && (self.max_channel() - self.min_channel()) > 30
    }
}

/// A rectangular pixel buffer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Image {
    width: usize,
    height: usize,
    pixels: Vec<u32>,
}

impl Image {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height],
        }
    }

    pub fn filled(width: usize, height: usize, colour: Rgb) -> Self {
        Self {
            width,
            height,
            pixels: vec![colour.to_u32(); width * height],
        }
    }

    pub fn from_pixels(width: usize, height: usize, pixels: Vec<u32>) -> Self {
        assert_eq!(width * height, pixels.len(), "pixel count does not match size");
        Self { width, height, pixels }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn as_slice(&self) -> &[u32] {
        &self.pixels
    }

    pub fn as_mut_slice(&mut self) -> &mut [u32] {
        &mut self.pixels
    }

    /// Pixel at an unsigned position; out of range reads as black.
    pub fn get(&self, x: usize, y: usize) -> Rgb {
        if x >= self.width || y >= self.height {
            return Rgb::default();
        }
        Rgb::from_u32(self.pixels[y * self.width + x])
    }

    /// Pixel at a signed position; out of range reads as black.  Gradient
    /// scoring looks one row above the border, so signed coordinates are the
    /// natural interface.
    pub fn at(&self, x: i32, y: i32) -> Rgb {
        if x < 0 || y < 0 {
            return Rgb::default();
        }
        self.get(x as usize, y as usize)
    }

    pub fn put(&mut self, x: usize, y: usize, colour: Rgb) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = colour.to_u32();
        }
    }

    pub fn put_i(&mut self, x: i32, y: i32, colour: Rgb) {
        if x < 0 || y < 0 {
            return;
        }
        self.put(x as usize, y as usize, colour);
    }

    pub fn fill(&mut self, colour: Rgb) {
        self.pixels.fill(colour.to_u32());
    }

    /// Copy a sub-rectangle; used by the fake screen to answer capture calls.
    pub fn sub_image(&self, x: i32, y: i32, w: usize, h: usize) -> Option<Image> {
        if x < 0 || y < 0 {
            return None;
        }
        let (x, y) = (x as usize, y as usize);
        if x + w > self.width || y + h > self.height {
            return None;
        }
        let mut out = Image::new(w, h);
        for row in 0..h {
            let src = (y + row) * self.width + x;
            let dst = row * w;
            out.pixels[dst..dst + w].copy_from_slice(&self.pixels[src..src + w]);
        }
        Some(out)
    }

    /// A coarse fingerprint of the picture: the average colour of each cell of a
    /// 32x32 grid.  Two grabs of the same palette agree on it; a different roll
    /// of colours does not.
    pub fn signature(&self) -> Vec<u8> {
        let side = SIGNATURE_SIDE;
        let mut out = Vec::with_capacity(side * side * 3);

        for gy in 0..side {
            for gx in 0..side {
                let x0 = gx * self.width / side;
                let x1 = (gx + 1) * self.width / side;
                let y0 = gy * self.height / side;
                let y1 = (gy + 1) * self.height / side;

                let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
                for y in y0..y1.max(y0 + 1) {
                    for x in x0..x1.max(x0 + 1) {
                        let px = self.get(x, y);
                        r += px.r as u32;
                        g += px.g as u32;
                        b += px.b as u32;
                        n += 1;
                    }
                }
                let n = n.max(1);
                out.push((r / n) as u8);
                out.push((g / n) as u8);
                out.push((b / n) as u8);
            }
        }

        out
    }
}

/// Side of the grid [`Image::signature`] averages over.
const SIGNATURE_SIDE: usize = 32;

/// How far two signatures may drift and still describe the same picture: room
/// for a stray pixel or a slightly different shade, not for a new roll.
pub const SIGNATURE_SLACK: u8 = 8;

/// Do two signatures describe the same picture, within a little rendering noise?
pub fn same_signature(a: &[u8], b: &[u8], tolerance: u8) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(one, other)| one.abs_diff(*other) <= tolerance)
}

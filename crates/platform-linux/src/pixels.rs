//! Turning an X11 image into the core's `0x00RRGGBB` pixels.
//!
//! `GetImage` hands back raw scanlines in the server's format: 16, 24 or 32
//! bits per pixel, either byte order, padded to the pixmap format's scanline
//! unit, with the colour channels wherever the root visual says they are.

/// How the X server lays out a pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelFormat {
    pub bits_per_pixel: u8,
    /// Scanline padding, in bits (32 on every server in practice).
    pub scanline_pad: u8,
    pub lsb_first: bool,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
}

impl PixelFormat {
    /// The format used by a typical TrueColor root: 32 bits per pixel stored
    /// as B,G,R,X on a little-endian machine.
    #[cfg(test)]
    pub const fn bgra32() -> Self {
        Self {
            bits_per_pixel: 32,
            scanline_pad: 32,
            lsb_first: true,
            red_mask: 0x00FF_0000,
            green_mask: 0x0000_FF00,
            blue_mask: 0x0000_00FF,
        }
    }

    fn channel(pixel: u32, mask: u32) -> u8 {
        if mask == 0 {
            return 0;
        }
        let shift = mask.trailing_zeros();
        let max = mask >> shift;
        let value = (pixel & mask) >> shift;
        if max == 0 {
            0
        } else {
            // Scale an arbitrary-width channel down to 8 bits, so 5- and
            // 6-bit channels are as correct as 8-bit ones.
            ((value as u64 * 255) / max as u64) as u8
        }
    }
}

/// Bytes each scanline takes, including the server's padding.
fn stride(width: usize, format: &PixelFormat) -> usize {
    let pad = format.scanline_pad.max(8) as usize;
    let bits = width * format.bits_per_pixel as usize;
    bits.div_ceil(pad) * pad / 8
}

/// Decode `data` into `width * height` pixels.
///
/// Returns `None` for a bit depth this decoder does not know or a buffer that
/// is too short, so a capture never turns into a picture of the wrong size.
pub fn decode(data: &[u8], width: usize, height: usize, format: &PixelFormat) -> Option<Vec<u32>> {
    let bytes_per_pixel = format.bits_per_pixel as usize / 8;
    if !(2..=4).contains(&bytes_per_pixel) || width == 0 || height == 0 {
        return None;
    }

    let stride = stride(width, format);
    if data.len() < stride.checked_mul(height)? {
        return None;
    }

    let mut out = Vec::with_capacity(width * height);
    for y in 0..height {
        let row = &data[y * stride..];
        for x in 0..width {
            let o = x * bytes_per_pixel;
            let pixel = match (bytes_per_pixel, format.lsb_first) {
                (2, true) => u16::from_le_bytes([row[o], row[o + 1]]) as u32,
                (2, false) => u16::from_be_bytes([row[o], row[o + 1]]) as u32,
                (3, true) => {
                    row[o] as u32 | ((row[o + 1] as u32) << 8) | ((row[o + 2] as u32) << 16)
                }
                (3, false) => {
                    ((row[o] as u32) << 16) | ((row[o + 1] as u32) << 8) | row[o + 2] as u32
                }
                (4, true) => u32::from_le_bytes([row[o], row[o + 1], row[o + 2], row[o + 3]]),
                (4, false) => u32::from_be_bytes([row[o], row[o + 1], row[o + 2], row[o + 3]]),
                _ => return None,
            };

            let r = PixelFormat::channel(pixel, format.red_mask);
            let g = PixelFormat::channel(pixel, format.green_mask);
            let b = PixelFormat::channel(pixel, format.blue_mask);
            out.push(((r as u32) << 16) | ((g as u32) << 8) | b as u32);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgra_bytes_become_rgb_pixels() {
        // 2x1: one red pixel, one blue pixel, as B,G,R,X on little-endian.
        let data = [0x00, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0x00, 0x00];
        let pixels = decode(&data, 2, 1, &PixelFormat::bgra32()).unwrap();
        assert_eq!(pixels, vec![0x00FF_0000, 0x0000_00FF]);
    }

    #[test]
    fn twenty_four_bit_pixels_are_read_low_byte_first() {
        // One pixel: B=0x33, G=0x22, R=0x11, then one padding byte.
        let data = [0x33, 0x22, 0x11, 0x00];
        let format = PixelFormat {
            bits_per_pixel: 24,
            ..PixelFormat::bgra32()
        };
        let pixels = decode(&data, 1, 1, &format).unwrap();
        assert_eq!(pixels, vec![0x0011_2233]);
    }

    #[test]
    fn big_endian_pixels_are_read_the_other_way() {
        let data = [0x00, 0x11, 0x22, 0x33];
        let format = PixelFormat {
            lsb_first: false,
            ..PixelFormat::bgra32()
        };
        let pixels = decode(&data, 1, 1, &format).unwrap();
        assert_eq!(pixels, vec![0x0011_2233]);
    }

    #[test]
    fn sixteen_bit_channels_are_scaled_up() {
        // RGB565 red: 0xF800.
        let data = [0x00, 0xF8];
        let format = PixelFormat {
            bits_per_pixel: 16,
            scanline_pad: 16,
            lsb_first: true,
            red_mask: 0xF800,
            green_mask: 0x07E0,
            blue_mask: 0x001F,
        };
        let pixels = decode(&data, 1, 1, &format).unwrap();
        assert_eq!(pixels, vec![0x00FF_0000]);
    }

    #[test]
    fn scanlines_are_padded() {
        // Two 24-bit pixels are 6 bytes; a 32-bit pad adds two bytes per row.
        let data = [
            0x00, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xAA, 0xAA, // red, green, pad
            0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xBB, 0xBB, // blue, red, pad
        ];
        let format = PixelFormat {
            bits_per_pixel: 24,
            ..PixelFormat::bgra32()
        };
        let pixels = decode(&data, 2, 2, &format).unwrap();
        assert_eq!(
            pixels,
            vec![0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0x00FF_0000]
        );
    }

    #[test]
    fn a_short_buffer_is_rejected() {
        assert!(decode(&[0, 0, 0], 2, 2, &PixelFormat::bgra32()).is_none());
    }
}

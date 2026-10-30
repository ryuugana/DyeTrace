//! Minimal 24/32-bit BMP reader and 24-bit writer.
//!
//! Only used by the replay harness and the diagnostics: a real backend hands
//! over pixels straight from the screen.

use dyetrace_core::image::{Image, Rgb};
use std::fs;
use std::io;
use std::path::Path;

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

fn u16_at(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}

fn u32_at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

fn i32_at(d: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

/// Read a BMP into an [`Image`].  Handles uncompressed 24- and 32-bit files,
/// both row orders.
pub fn read_bmp(path: &Path) -> io::Result<Image> {
    let data = fs::read(path)?;
    if data.len() < 54 || &data[0..2] != b"BM" {
        return Err(bad("not a BMP file"));
    }

    let pixel_offset = u32_at(&data, 10) as usize;
    let width = i32_at(&data, 18);
    let height = i32_at(&data, 22);
    let bpp = u16_at(&data, 28);
    let compression = u32_at(&data, 30);

    if compression != 0 {
        return Err(bad("compressed BMP is not supported"));
    }
    if bpp != 24 && bpp != 32 {
        return Err(bad("only 24- and 32-bit BMP is supported"));
    }
    if width <= 0 || height == 0 {
        return Err(bad("empty BMP"));
    }

    let w = width as usize;
    let h = height.unsigned_abs() as usize;
    let top_down = height < 0;
    let bytes_per_pixel = bpp as usize / 8;
    let stride = (w * bytes_per_pixel + 3) & !3;

    let mut img = Image::new(w, h);
    for row in 0..h {
        let y = if top_down { row } else { h - 1 - row };
        let base = pixel_offset + row * stride;
        if base + w * bytes_per_pixel > data.len() {
            return Err(bad("truncated BMP"));
        }
        for x in 0..w {
            let o = base + x * bytes_per_pixel;
            img.put(x, y, Rgb::new(data[o + 2], data[o + 1], data[o]));
        }
    }

    Ok(img)
}

/// Write an [`Image`] as an uncompressed 24-bit, top-down BMP.
pub fn write_bmp(path: &Path, img: &Image) -> io::Result<()> {
    let w = img.width();
    let h = img.height();
    let stride = (w * 3 + 3) & !3;
    let image_size = stride * h;
    let header_size = 54usize;

    let mut out = vec![0u8; header_size + image_size];
    out[0..2].copy_from_slice(b"BM");
    out[2..6].copy_from_slice(&((header_size + image_size) as u32).to_le_bytes());
    out[10..14].copy_from_slice(&(header_size as u32).to_le_bytes());
    out[14..18].copy_from_slice(&40u32.to_le_bytes());
    out[18..22].copy_from_slice(&(w as i32).to_le_bytes());
    out[22..26].copy_from_slice(&(-(h as i32)).to_le_bytes());
    out[26..28].copy_from_slice(&1u16.to_le_bytes());
    out[28..30].copy_from_slice(&24u16.to_le_bytes());
    out[34..38].copy_from_slice(&(image_size as u32).to_le_bytes());

    for y in 0..h {
        let row = header_size + y * stride;
        for x in 0..w {
            let p = img.get(x, y);
            let o = row + x * 3;
            out[o] = p.b;
            out[o + 1] = p.g;
            out[o + 2] = p.r;
        }
    }

    fs::write(path, out)
}

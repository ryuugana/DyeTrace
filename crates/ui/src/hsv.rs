//! Colour space conversions used by the picker.

/// Hue in degrees `0..360`, saturation and value in `0..1`.
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let hue = if delta <= f32::EPSILON {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };

    let saturation = if max <= f32::EPSILON { 0.0 } else { delta / max };
    (hue.rem_euclid(360.0), saturation, max)
}

pub fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> (u8, u8, u8) {
    let hue = hue.rem_euclid(360.0);
    let saturation = saturation.clamp(0.0, 1.0);
    let value = value.clamp(0.0, 1.0);

    let c = value * saturation;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = value - c;

    let (r, g, b) = match hue as u32 / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    let to_byte = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to_byte(r), to_byte(g), to_byte(b))
}

/// Full saturation/value colour for a hue, used to draw the picker.
pub fn hue_colour(hue: f32) -> (u8, u8, u8) {
    hsv_to_rgb(hue, 1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_primary_colours() {
        for colour in [(0, 0, 0), (255, 255, 255), (255, 0, 0), (0, 255, 0), (0, 0, 255)] {
            let (h, s, v) = rgb_to_hsv(colour.0, colour.1, colour.2);
            assert_eq!(hsv_to_rgb(h, s, v), colour);
        }
    }

    #[test]
    fn round_trips_a_muted_colour() {
        let colour = (120, 60, 200);
        let (h, s, v) = rgb_to_hsv(colour.0, colour.1, colour.2);
        assert_eq!(hsv_to_rgb(h, s, v), colour);
    }
}

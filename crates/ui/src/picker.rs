//! Saturation/value square, hue strip and preview swatch.

use crate::hsv;
use dyetrace_core::image::Rgb;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, TextureHandle, Ui, Vec2};

/// Textures are rebuilt only when the hue (for the square) or nothing (for the
/// strip) changes.
pub struct PickerTextures {
    square: Option<TextureHandle>,
    square_hue: f32,
    strip: Option<TextureHandle>,
}

impl Default for PickerTextures {
    fn default() -> Self {
        Self {
            square: None,
            square_hue: -1.0,
            strip: None,
        }
    }
}

fn color_image(width: usize, height: usize, pixel: impl Fn(usize, usize) -> Color32) -> egui::ColorImage {
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            pixels.push(pixel(x, y));
        }
    }
    egui::ColorImage {
        size: [width, height],
        pixels,
    }
}

impl PickerTextures {
    fn square_for(&mut self, ctx: &egui::Context, hue: f32) -> &TextureHandle {
        if self.square.is_none() || (self.square_hue - hue).abs() > 0.5 {
            let image = color_image(96, 72, |x, y| {
                let s = x as f32 / 95.0;
                let v = 1.0 - y as f32 / 71.0;
                let (r, g, b) = hsv::hsv_to_rgb(hue, s, v);
                Color32::from_rgb(r, g, b)
            });
            self.square = Some(ctx.load_texture("sv-square", image, Default::default()));
            self.square_hue = hue;
        }
        self.square.as_ref().unwrap()
    }

    fn strip(&mut self, ctx: &egui::Context) -> &TextureHandle {
        if self.strip.is_none() {
            let image = color_image(256, 1, |x, _| {
                let (r, g, b) = hsv::hue_colour(x as f32 / 255.0 * 360.0);
                Color32::from_rgb(r, g, b)
            });
            self.strip = Some(ctx.load_texture("hue-strip", image, Default::default()));
        }
        self.strip.as_ref().unwrap()
    }
}

fn uv_full() -> Rect {
    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0))
}

/// Draw the square.  Returns true when the user changed the colour.
pub fn saturation_value_square(
    ui: &mut Ui,
    textures: &mut PickerTextures,
    size: Vec2,
    hue: &mut f32,
    saturation: &mut f32,
    value: &mut f32,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let texture = textures.square_for(ui.ctx(), *hue);
    ui.painter().image(texture.id(), rect, uv_full(), Color32::WHITE);

    // crosshair at the current colour
    let marker = Pos2::new(
        rect.left() + saturation.clamp(0.0, 1.0) * rect.width(),
        rect.top() + (1.0 - value.clamp(0.0, 1.0)) * rect.height(),
    );
    ui.painter().circle_stroke(marker, 5.0, Stroke::new(1.5_f32, Color32::WHITE));
    ui.painter().circle_stroke(marker, 6.5, Stroke::new(1.0_f32, Color32::BLACK));

    let mut changed = false;
    if let Some(pos) = response.interact_pointer_pos() {
        if response.clicked() || response.dragged() {
            *saturation = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            *value = (1.0 - (pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
            changed = true;
        }
    }
    let _ = hue;
    changed
}

/// Draw the hue strip.  Returns true when the hue changed.
pub fn hue_strip(
    ui: &mut Ui,
    textures: &mut PickerTextures,
    size: Vec2,
    hue: &mut f32,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let texture = textures.strip(ui.ctx());
    ui.painter().image(texture.id(), rect, uv_full(), Color32::WHITE);

    let x = rect.left() + (hue.rem_euclid(360.0) / 360.0) * rect.width();
    let marker = Rect::from_min_max(
        Pos2::new(x - 1.5, rect.top() - 2.0),
        Pos2::new(x + 1.5, rect.bottom() + 2.0),
    );
    ui.painter().rect_filled(marker, 1.0, Color32::WHITE);
    ui.painter().line_segment(
        [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
        Stroke::new(2.0_f32, Color32::WHITE),
    );
    ui.painter().line_segment(
        [Pos2::new(x + 1.0, rect.top()), Pos2::new(x + 1.0, rect.bottom())],
        Stroke::new(1.0_f32, Color32::BLACK),
    );

    if let Some(pos) = response.interact_pointer_pos() {
        if response.clicked() || response.dragged() {
            *hue = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) * 359.9;
            return true;
        }
    }
    false
}

/// A filled rectangle showing the current colour.
pub fn swatch(ui: &mut Ui, size: Vec2, colour: Rgb) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, 4.0, Color32::from_rgb(colour.r, colour.g, colour.b));
    ui.painter().rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, Color32::from_gray(110)),
        egui::StrokeKind::Inside,
    );
}

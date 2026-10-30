//! The window: controls on the left, colour picker and match preview on the
//! right.

use crate::hotkey::{self, StopKey};
use crate::hsv;
use crate::picker::{self, PickerTextures};
use crate::presets::Presets;
use crate::preview::Preview;
use crate::worker::{self, Job, Msg, Worker};

use dyetrace_core::image::{Image, Rgb};
use dyetrace_core::search;
use dyetrace_engine::cycle::{Options, Scan};

use eframe::egui::{self, Color32, RichText, Sense, Stroke, TextureHandle, Vec2};

/// What a scan measured on one roll of the palette.
struct Offsets {
    pointer: (i32, i32),
    table: Vec<(i32, i32)>,
    slots: Vec<usize>,
    boxes: Vec<(i32, i32)>,
    best: Option<search::Match>,
}

/// A fingerprint of a palette, for the texture cache.
fn signature_key(signature: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in signature {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

pub struct DyeTraceApp {
    colour: Rgb,
    hex_text: String,
    hue: f32,
    saturation: f32,
    value: f32,

    slots: usize,
    variance: u8,
    /// Rolls Auto may try after the first before giving up.
    retries: usize,
    /// Slots that must hold the colour, numbered as the dye window does.  Empty
    /// falls back to `slots`.
    slots_only: Vec<usize>,

    presets: Presets,
    save_name: String,

    textures: PickerTextures,
    hit_texture: Option<TextureHandle>,
    hit_key: (u32, u8, u64),

    preview: Preview,
    worker: Worker,
    stop_key: StopKey,
    /// The key the player picked to stop a cycle.
    stop_on: egui::Key,
    /// Waiting for the player to press the key they want.
    recording: bool,
    /// Is the About window open?
    about: bool,
    scan: Option<Scan>,
    status: String,

}

impl DyeTraceApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // The preview is a dark backdrop with white/red hits, so the window is
        // dark whatever the desktop theme is doing.
        cc.egui_ctx
            .options_mut(|options| options.theme_preference = egui::ThemePreference::Dark);

        let preview = Preview::spawn();
        let worker = Worker::spawn(&preview);
        let stop_key = StopKey::watch(worker.stop_flag().expect("worker stop flag"));

        let mut app = Self {
            colour: Rgb::black(),
            hex_text: "000000".to_string(),
            hue: 0.0,
            saturation: 0.0,
            value: 0.0,
            slots: 1,
            variance: 0,
            retries: 20,
            slots_only: Vec::new(),
            presets: Presets::load(),
            save_name: String::new(),
            textures: PickerTextures::default(),
            hit_texture: None,
            hit_key: (u32::MAX, 0, 0),
            preview,
            worker,
            stop_key,
            stop_on: hotkey::default_key(),
            recording: false,
            about: false,
            scan: None,
            status: "watching for the dye window".to_string(),
        };
        app.set_colour(Rgb::black());
        app
    }

    fn set_colour(&mut self, colour: Rgb) {
        self.colour = colour;
        self.hex_text = worker::hex(colour);
        let (h, s, v) = hsv::rgb_to_hsv(colour.r, colour.g, colour.b);
        // Keep the hue when the colour is grey, otherwise the picker jumps.
        if s > 0.001 && v > 0.001 {
            self.hue = h;
        }
        self.saturation = s;
        self.value = v;
    }

    /// Take the next key the player presses as the stop key.
    fn capture_stop_key(&mut self, ctx: &egui::Context) {
        let pressed = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    ..
                } => Some(*key),
                _ => None,
            })
        });

        let Some(key) = pressed else { return };
        if let Some(code) = hotkey::virtual_key(key) {
            self.stop_on = key;
            self.stop_key.set(code);
            self.recording = false;
        }
    }

    fn colour_from_picker(&mut self) {
        let (r, g, b) = hsv::hsv_to_rgb(self.hue, self.saturation, self.value);
        self.colour = Rgb::new(r, g, b);
        self.hex_text = worker::hex(self.colour);
    }

    fn options(&self, dry_run: bool) -> Options {
        Options {
            colour: self.colour,
            variance: self.variance,
            slots: self.slots,
            slots_only: self.slots_only.clone(),
            // One roll is the attempt, the rest are retries; 0 keeps going.
            max_cycles: match self.retries {
                0 => usize::MAX,
                retries => retries.saturating_add(1),
            },
            dry_run,
            stop: self.worker.stop_flag(),
            ..Options::default()
        }
    }

    fn drain_worker(&mut self, ctx: &egui::Context) {
        for message in self.worker.poll() {
            match message {
                Msg::Status(text) => self.status = text,
                Msg::Scanned(scan) => {
                    self.hit_key = (u32::MAX, 0, 0);
                    self.scan = Some(*scan);
                }
                Msg::Finished => {
                    // the job may have clicked inside the client, which gives it
                    // focus; come back to the front when it is done
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
        }
        if self.worker.busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
    }

    fn ensure_hit_texture(&mut self, ctx: &egui::Context, grid: &Image, signature: &[u8]) {
        let key = (self.colour.to_u32(), self.variance, signature_key(signature));
        if self.hit_texture.is_some() && key == self.hit_key {
            return;
        }

        let map = search::hit_map(grid, self.colour, self.variance);
        let mut pixels = Vec::with_capacity(map.len());
        for value in map {
            let (r, g, b) = (
                ((value >> 16) & 0xFF) as u8,
                ((value >> 8) & 0xFF) as u8,
                (value & 0xFF) as u8,
            );
            pixels.push(Color32::from_rgb(r, g, b));
        }
        let image = egui::ColorImage {
            size: [dyetrace_core::SCAN_W, dyetrace_core::SCAN_H],
            pixels,
        };
        self.hit_texture = Some(ctx.load_texture("hit-map", image, Default::default()));
        self.hit_key = key;
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(RichText::new("Selected colour").strong());
        let mut picked: Option<usize> = None;
        ui.horizontal(|ui| {
            let width = ui.available_width() - 86.0;
            egui::ComboBox::from_id_salt("presets")
                .width(width.max(120.0))
                .selected_text(
                    self.presets
                        .selected()
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "custom".to_string()),
                )
                .show_ui(ui, |ui| {
                    for (index, preset) in self.presets.items.iter().enumerate() {
                        if ui
                            .selectable_label(
                                self.presets.selected == index,
                                format!("{:<12} #{}", preset.name, worker::hex(preset.colour)),
                            )
                            .clicked()
                        {
                            picked = Some(index);
                        }
                    }
                });
            if ui
                .add_sized([78.0, 20.0], egui::Button::new("Save as…"))
                .clicked()
            {
                let name = self.save_name.clone();
                let colour = self.colour;
                self.presets.save(&name, colour);
                self.save_name.clear();
            }
        });
        if let Some(index) = picked {
            self.presets.selected = index;
            let colour = self.presets.items[index].colour;
            self.set_colour(colour);
        }
        ui.horizontal(|ui| {
            ui.add_sized(
                [64.0, 18.0],
                egui::TextEdit::singleline(&mut self.save_name).hint_text("name"),
            );
            picker::swatch(ui, Vec2::new(42.0, 18.0), self.colour);
            ui.label(RichText::new("saved into presets.txt").weak().small());
        });

        ui.add_space(10.0);
        ui.label(RichText::new("RGB").strong());
        ui.horizontal(|ui| {
            let mut changed = false;
            for (label, channel) in [("R", &mut self.colour.r), ("G", &mut self.colour.g), ("B", &mut self.colour.b)] {
                ui.label(label);
                if ui
                    .add_sized([58.0, 20.0], egui::DragValue::new(channel).range(0..=255).speed(1.0))
                    .changed()
                {
                    changed = true;
                }
            }
            if changed {
                let colour = self.colour;
                self.set_colour(colour);
            }
        });

        ui.horizontal(|ui| {
            ui.label("Hex");
            let response = ui.add_sized(
                [96.0, 20.0],
                egui::TextEdit::singleline(&mut self.hex_text).char_limit(6),
            );
            if response.changed() {
                let text = self.hex_text.trim().trim_start_matches('#').to_string();
                if text.len() == 6 {
                    if let Ok(value) = u32::from_str_radix(&text, 16) {
                        self.set_colour(Rgb::from_u32(value));
                        self.hex_text = text.to_uppercase();
                    }
                }
            }
            ui.label(RichText::new("(web)").weak());
        });

        self.picker_panel(ui);

        ui.add_space(14.0);
        let busy = self.worker.busy;
        let side = ui.available_width().min(120.0);

        ui.horizontal(|ui| {
            let row = ui.cursor().min.x;

            let mut start = false;
            let mut about = false;
            ui.add_enabled_ui(!busy, |ui| {
                start = ui
                    .add_sized([58.0, 26.0], egui::Button::new("Start"))
                    .clicked();
                about = ui
                    .add_sized([58.0, 26.0], egui::Button::new("About"))
                    .clicked();
            });

            // The stop key sits in the same column as the numbers above it.
            let want = row + side + 16.0;
            let here = ui.cursor().min.x;
            if want > here {
                ui.add_space(want - here);
            }

            ui.label("Stop key");
            if ui.add_sized([40.0, 26.0], egui::Button::new("Set")).clicked() {
                self.recording = true;
            }
            let shown = if self.recording {
                "press a key".to_string()
            } else {
                self.stop_on.name().to_string()
            };
            ui.label(RichText::new(shown).strong().size(15.0));

            if start {
                self.worker.submit(Job::Auto(self.options(false)));
            }
            if about {
                self.about = true;
            }
        });
    }

    /// The colour picker, directly under the hex box, with the matching
    /// controls beside it.
    fn picker_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        let side = ui.available_width().min(120.0);

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                let mut changed = picker::saturation_value_square(
                    ui,
                    &mut self.textures,
                    Vec2::splat(side),
                    &mut self.hue,
                    &mut self.saturation,
                    &mut self.value,
                );
                ui.add_space(4.0);
                changed |= picker::hue_strip(
                    ui,
                    &mut self.textures,
                    Vec2::new(side, 14.0),
                    &mut self.hue,
                );
                if changed {
                    self.colour_from_picker();
                }
            });

            ui.add_space(10.0);
            ui.vertical(|ui| self.match_controls(ui));
        });
    }

    /// Slot selection, dye spots and variance, sitting beside the picker.
    fn match_controls(&mut self, ui: &mut egui::Ui) {
        ui.label("Slot");
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            let mut chosen = self.slots_only.clone();
            for n in 1..=5usize {
                let slot = n - 1;
                let selected = chosen.contains(&slot);
                let button = egui::Button::new(format!("{n}")).selected(selected);
                if ui.add_sized([22.0, 20.0], button).clicked() {
                    if selected {
                        chosen.retain(|picked| *picked != slot);
                    } else {
                        chosen.push(slot);
                        chosen.sort();
                    }
                }
            }
            self.slots_only = chosen;
        });

        ui.add_space(8.0);
        egui::Grid::new("match-controls")
            .num_columns(2)
            .spacing([6.0, 4.0])
            .show(ui, |ui| {
                ui.label("Dye spots");
                ui.add_sized(
                    [46.0, 18.0],
                    egui::DragValue::new(&mut self.slots).range(1..=5).speed(0.1),
                );
                ui.end_row();

                ui.label("Retries");
                ui.add_sized(
                    [46.0, 18.0],
                    egui::DragValue::new(&mut self.retries).range(0..=99).speed(0.1),
                );
                ui.end_row();

                ui.label("Variance ±");
                ui.add_sized(
                    [46.0, 18.0],
                    egui::DragValue::new(&mut self.variance).range(0..=255).speed(1.0),
                );
                ui.end_row();
            });

        ui.add_space(3.0);
        ui.spacing_mut().slider_width = (ui.available_width() - 10.0).max(60.0);
        ui.add(egui::Slider::new(&mut self.variance, 0..=255).show_value(false));
    }

    fn preview_panel(&mut self, ui: &mut egui::Ui) {
        let snapshot = self.preview.latest();
        if let Some(snapshot) = &snapshot {
            self.ensure_hit_texture(ui.ctx(), &snapshot.grid, &snapshot.signature);
        }

        // The offsets belong to the roll they were measured on: once the palette
        // is rolled again they mean nothing and are not drawn.
        let offsets = match (&snapshot, &self.scan) {
            (Some(snapshot), Some(scan)) if snapshot.shows(&scan.signature) => Some(Offsets {
                pointer: scan.pointer,
                table: scan.table.clone(),
                slots: scan.slots.clone(),
                boxes: scan.boxes.clone(),
                best: scan.best,
            }),
            _ => None,
        };

        let available = ui.available_size();
        let side = available.x.min(available.y).clamp(160.0, 460.0);
        // The same air above the picture as below it, down to the status bar.
        ui.add_space(((available.y - side) * 0.5).max(0.0));
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());

        if snapshot.is_none() {
            ui.painter().rect_filled(rect, 2.0, Color32::from_gray(28));
            ui.painter().rect_stroke(
                rect,
                2.0,
                Stroke::new(1.0_f32, Color32::from_gray(70)),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "waiting for the dye window",
                egui::FontId::proportional(13.0),
                Color32::from_gray(140),
            );
            return;
        }

        if let Some(texture) = &self.hit_texture {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter()
                .image(texture.id(), rect, uv, Color32::WHITE);
        }

        let Some(offsets) = offsets else {
            return;
        };

        let scale = rect.width() / dyetrace_core::SCAN_W as f32;
        let to_screen = |x: i32, y: i32| {
            egui::pos2(
                rect.left() + x as f32 * scale,
                rect.top() + y as f32 * scale,
            )
        };

        // legend boxes the client drew
        for (bx, by) in &offsets.boxes {
            let centre = to_screen(*bx, *by);
            ui.painter().rect_stroke(
                egui::Rect::from_center_size(centre, Vec2::splat(5.0)),
                0.0,
                Stroke::new(1.0_f32, Color32::from_rgb(0, 200, 255)),
                egui::StrokeKind::Inside,
            );
        }

        // sample offsets around the pointer, numbered to match the slot picker
        for (index, (dx, dy)) in offsets.table.iter().enumerate() {
            let slot = offsets.slots.get(index).copied().unwrap_or(index);
            let point = to_screen(offsets.pointer.0 + dx, offsets.pointer.1 + dy);
            if !rect.contains(point) {
                continue;
            }
            let chosen = self.slots_only.contains(&slot);
            let colour = if chosen {
                Color32::from_rgb(20, 90, 220)
            } else {
                Color32::from_gray(120)
            };
            let width = if chosen { 2.0_f32 } else { 1.0_f32 };
            ui.painter().circle_stroke(
                point,
                if chosen { 5.0 } else { 2.5 },
                Stroke::new(width, colour),
            );
            ui.painter().text(
                point + Vec2::new(5.0, -5.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{}", slot + 1),
                egui::FontId::proportional(10.0),
                colour,
            );
        }

        // the best position
        if let Some(best) = offsets.best {
            let centre = to_screen(best.x, best.y);
            ui.painter()
                .circle_stroke(centre, 7.0, Stroke::new(2.0_f32, Color32::from_rgb(0, 255, 140)));
            ui.painter().line_segment(
                [centre + Vec2::new(-11.0, 0.0), centre + Vec2::new(11.0, 0.0)],
                Stroke::new(1.0_f32, Color32::from_rgb(0, 255, 140)),
            );
            ui.painter().line_segment(
                [centre + Vec2::new(0.0, -11.0), centre + Vec2::new(0.0, 11.0)],
                Stroke::new(1.0_f32, Color32::from_rgb(0, 255, 140)),
            );
        }
    }

    /// What the tool is, and how to use it.
    fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about {
            return;
        }

        let mut open = true;
        egui::Window::new("About DyeTrace")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(340.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("An open-source dye palette finder for Mabinogi").strong(),
                );
                ui.add_space(6.0);
                ui.label("Open the Dye window in game and leave it visible.");
                ui.label(
                    "The picture on the right is the palette: white where the colour is not",
                );
                ui.label("there, red where it is.");
                ui.label("1. Pick the colour: RGB, hex, or the square.");
                ui.label("2. Dye spots is how many of the five samples must hold it;");
                ui.label("   Slot pins particular samples the dye window numbers 1 to 5.");
                ui.label("3. Start rolls the dye until it finds a place and clicks it.");
                ui.label("4. The stop key ends a run; Set records a different one.");
                ui.add_space(8.0);
                ui.hyperlink_to(
                    "github.com/ryuugana/DyeTrace",
                    "https://github.com/ryuugana/DyeTrace",
                );
                ui.label(RichText::new("MIT licensed.").weak().small());
            });

        self.about = open;
    }

    /// The one line of feedback, along the bottom edge of the window.
    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if self.worker.busy {
                ui.spinner();
            }
            ui.label(RichText::new(&self.status).strong());
        });
        ui.add_space(6.0);
    }

}

impl eframe::App for DyeTraceApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_worker(ctx);

        if self.recording {
            self.capture_stop_key(ctx);
        }

        // The palette is watched on its own thread; wake up often enough to show
        // what it found.
        ctx.request_repaint_after(std::time::Duration::from_millis(250));

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            self.status_bar(ui);
        });

        egui::SidePanel::left("controls")
            .resizable(false)
            .default_width(340.0)
            .exact_width(310.0)
            .show(ctx, |ui| {
                self.controls(ui);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| self.preview_panel(ui));
        });

        self.about_window(ctx);
    }
}

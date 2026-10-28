//! The window: controls on the left, colour picker and match preview on the
//! right.

use crate::hsv;
use crate::picker::{self, PickerTextures};
use crate::presets::Presets;
use crate::worker::{self, Job, Msg, Worker};

use dyetrace_core::image::Rgb;
use dyetrace_engine::cycle::{Options, Scan};

use eframe::egui::{self, Color32, RichText, Sense, Stroke, TextureHandle, Vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Dye,
    Advanced,
}

pub struct DyeTraceApp {
    colour: Rgb,
    hex_text: String,
    hue: f32,
    saturation: f32,
    value: f32,

    slots: usize,
    variance: u8,

    presets: Presets,
    save_name: String,

    textures: PickerTextures,
    hit_texture: Option<TextureHandle>,
    hit_key: (u32, u8),

    worker: Worker,
    scan: Option<Scan>,
    status: String,
    log: Vec<String>,
    tab: Tab,
}

impl DyeTraceApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            colour: Rgb::black(),
            hex_text: "000000".to_string(),
            hue: 0.0,
            saturation: 0.0,
            value: 0.0,
            slots: 1,
            variance: 0,
            presets: Presets::load(),
            save_name: String::new(),
            textures: PickerTextures::default(),
            hit_texture: None,
            hit_key: (u32::MAX, 0),
            worker: Worker::spawn(),
            scan: None,
            status: "open the dye window in game, then press Find".to_string(),
            log: Vec::new(),
            tab: Tab::Dye,
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
            dry_run,
            stop: self.worker.stop_flag(),
            ..Options::default()
        }
    }

    fn drain_worker(&mut self, ctx: &egui::Context) {
        for message in self.worker.poll() {
            match message {
                Msg::Status(text) => self.status = text,
                Msg::Log(line) => {
                    self.log.push(line);
                    if self.log.len() > 200 {
                        self.log.remove(0);
                    }
                }
                Msg::Scanned(scan) => {
                    self.hit_key = (u32::MAX, 0);
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

    fn ensure_hit_texture(&mut self, ctx: &egui::Context) {
        let Some(scan) = &self.scan else { return };
        let key = (self.colour.to_u32(), self.variance);
        if self.hit_texture.is_some() && key == self.hit_key {
            return;
        }

        let map = scan.hit_map(self.colour, self.variance);
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

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label("Dye spots");
            ui.add(egui::DragValue::new(&mut self.slots).range(1..=5).speed(0.1));
            ui.label(RichText::new("how many of the five samples").weak().small());
        });
        ui.horizontal(|ui| {
            ui.label("Variance ±");
            ui.add(egui::DragValue::new(&mut self.variance).range(0..=255).speed(1.0));
            ui.label(RichText::new("per channel").weak().small());
        });

        ui.add_space(14.0);
        let busy = self.worker.busy;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!busy, egui::Button::new("Find field"))
                .clicked()
            {
                self.worker.submit(Job::Find);
            }
            if ui.add_enabled(!busy, egui::Button::new("Scan")).clicked() {
                self.worker.submit(Job::Scan(self.options(true)));
            }
            if ui.add_enabled(!busy, egui::Button::new("Auto")).clicked() {
                self.worker.submit(Job::Auto(self.options(false)));
            }
            if ui.add_enabled(busy, egui::Button::new("Stop")).clicked() {
                self.worker.request_stop();
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.label(RichText::new(&self.status).strong());
    }

    fn picker_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Colour").strong());
        let width = ui.available_width().min(300.0);

        let mut changed = picker::saturation_value_square(
            ui,
            &mut self.textures,
            Vec2::new(width, width * 0.33),
            &mut self.hue,
            &mut self.saturation,
            &mut self.value,
        );

        ui.add_space(4.0);
        changed |= picker::hue_strip(
            ui,
            &mut self.textures,
            Vec2::new(width, 14.0),
            &mut self.hue,
        );

        if changed {
            self.colour_from_picker();
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            picker::swatch(ui, Vec2::new(48.0, 24.0), self.colour);
            ui.label(format!("#{}", worker::hex(self.colour)));
        });
    }

    fn preview_panel(&mut self, ui: &mut egui::Ui) {
        self.ensure_hit_texture(ui.ctx());

        let available = ui.available_size();
        let side = available.x.min(available.y - 24.0).clamp(160.0, 460.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());

        let Some(scan) = &self.scan else {
            ui.painter().rect_filled(rect, 2.0, Color32::from_gray(28));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "press Scan to search the gradient",
                egui::FontId::proportional(13.0),
                Color32::from_gray(140),
            );
            return;
        };

        if let Some(texture) = &self.hit_texture {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter()
                .image(texture.id(), rect, uv, Color32::WHITE);
        }

        let scale = rect.width() / dyetrace_core::SCAN_W as f32;
        let to_screen = |x: i32, y: i32| {
            egui::pos2(
                rect.left() + x as f32 * scale,
                rect.top() + y as f32 * scale,
            )
        };

        // legend boxes the client drew
        for (bx, by) in &scan.boxes {
            let centre = to_screen(*bx, *by);
            ui.painter().rect_stroke(
                egui::Rect::from_center_size(centre, Vec2::splat(5.0)),
                0.0,
                Stroke::new(1.0_f32, Color32::from_rgb(0, 200, 255)),
                egui::StrokeKind::Inside,
            );
        }

        // sample offsets around the pointer
        for (dx, dy) in &scan.table {
            let point = to_screen(scan.pointer.0 + dx, scan.pointer.1 + dy);
            if rect.contains(point) {
                ui.painter()
                    .circle_stroke(point, 2.5, Stroke::new(1.0_f32, Color32::from_gray(120)));
            }
        }

        // the best position
        if let Some(best) = scan.best {
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

    fn log_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, true])
            .max_height(64.0)
            .show(ui, |ui| {
                for line in &self.log {
                    ui.label(RichText::new(line).monospace().small().weak());
                }
            });
    }

    fn advanced(&mut self, ui: &mut egui::Ui) {
        ui.add_space(12.0);
        ui.heading("Advanced");
        ui.add_space(6.0);
        ui.label(
            "Reserved for the parts the reference build kept out of the way: button offsets, \
             capture timing, a manual slot table and a log view.",
        );
        ui.add_space(12.0);
        ui.label(RichText::new("Planned").strong());
        ui.label("• Pick Color / Cancel offsets (currently 147 / 231 / 397)");
        ui.label("• settle and retry timings");
        ui.label("• per-slot colours instead of one colour for all five");
        ui.label("• saved scan history and an export of the hit map");
    }
}

impl eframe::App for DyeTraceApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_worker(ctx);

        egui::TopBottomPanel::top("head").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("DyeTrace");
                ui.add_space(12.0);
                if ui
                    .selectable_label(self.tab == Tab::Dye, "Dye")
                    .clicked()
                {
                    self.tab = Tab::Dye;
                }
                if ui
                    .selectable_label(self.tab == Tab::Advanced, "Advanced")
                    .clicked()
                {
                    self.tab = Tab::Advanced;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.worker.busy {
                        ui.spinner();
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("log").show(ctx, |ui| {
            self.log_panel(ui);
        });

        if self.tab == Tab::Dye {
            egui::SidePanel::left("controls")
                .resizable(false)
                .exact_width(310.0)
                .show(ctx, |ui| {
                    self.controls(ui);

                    // Keep the picker at the bottom of the column.
                    let keep = 168.0;
                    let slack = (ui.available_height() - keep).max(0.0);
                    ui.add_space(slack);
                    self.picker_panel(ui);
                });

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| self.preview_panel(ui));
            });
        } else {
            egui::CentralPanel::default().show(ctx, |ui| self.advanced(ui));
        }
    }
}

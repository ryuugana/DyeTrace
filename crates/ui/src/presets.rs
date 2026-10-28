//! Saved colours: a small named palette, persisted next to the executable.

use dyetrace_core::image::Rgb;
use std::fs;
use std::path::PathBuf;

const FILE_NAME: &str = "presets.txt";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    pub name: String,
    pub colour: Rgb,
}

#[derive(Clone, Debug, Default)]
pub struct Presets {
    pub items: Vec<Preset>,
    pub selected: usize,
}

fn path() -> PathBuf {
    PathBuf::from(FILE_NAME)
}

impl Presets {
    /// Defaults plus whatever was saved last time.
    pub fn load() -> Self {
        let mut items = vec![
            Preset {
                name: "Black".to_string(),
                colour: Rgb::new(0, 0, 0),
            },
            Preset {
                name: "White".to_string(),
                colour: Rgb::new(255, 255, 255),
            },
        ];

        if let Ok(text) = fs::read_to_string(path()) {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let Some((name, hex)) = line.split_once('\t') else {
                    continue;
                };
                let Ok(value) = u32::from_str_radix(hex.trim().trim_start_matches('#'), 16) else {
                    continue;
                };
                let colour = Rgb::from_u32(value);
                if !items.iter().any(|p| p.name == name && p.colour == colour) {
                    items.push(Preset {
                        name: name.to_string(),
                        colour,
                    });
                }
            }
        }

        Self { items, selected: 0 }
    }

    pub fn selected(&self) -> Option<&Preset> {
        self.items.get(self.selected)
    }

    /// Add (or update) a named colour and persist the list.
    pub fn save(&mut self, name: &str, colour: Rgb) {
        let name = name.trim();
        let name = if name.is_empty() {
            format!("#{:02X}{:02X}{:02X}", colour.r, colour.g, colour.b)
        } else {
            name.to_string()
        };

        if let Some(existing) = self.items.iter_mut().find(|p| p.name == name) {
            existing.colour = colour;
            self.selected = self
                .items
                .iter()
                .position(|p| p.name == name)
                .unwrap_or(self.selected);
        } else {
            self.items.push(Preset { name, colour });
            self.selected = self.items.len() - 1;
        }

        self.persist();
    }

    fn persist(&self) {
        let mut text = String::from("# name\tRRGGBB\n");
        for preset in &self.items {
            text.push_str(&format!(
                "{}\t{:02X}{:02X}{:02X}\n",
                preset.name, preset.colour.r, preset.colour.g, preset.colour.b
            ));
        }
        let _ = fs::write(path(), text);
    }
}

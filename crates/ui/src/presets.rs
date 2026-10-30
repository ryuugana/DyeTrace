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
}

fn path() -> PathBuf {
    crate::config::data_dir().join(FILE_NAME)
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

        Self { items }
    }

    /// The saved name for a colour, or none if it has never been saved.
    ///
    /// The latest name wins, so saving a colour again under a new name shows
    /// that name.
    pub fn name_of(&self, colour: Rgb) -> Option<&str> {
        self.items
            .iter()
            .rev()
            .find(|preset| preset.colour == colour)
            .map(|preset| preset.name.as_str())
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
        } else {
            self.items.push(Preset { name, colour });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn presets() -> Presets {
        Presets {
            items: vec![
                Preset {
                    name: "Black".to_string(),
                    colour: Rgb::new(0, 0, 0),
                },
                Preset {
                    name: "White".to_string(),
                    colour: Rgb::new(255, 255, 255),
                },
            ],
        }
    }

    #[test]
    fn a_saved_colour_knows_its_name() {
        assert_eq!(presets().name_of(Rgb::new(0, 0, 0)), Some("Black"));
        assert_eq!(presets().name_of(Rgb::new(255, 255, 255)), Some("White"));
    }

    #[test]
    fn an_unsaved_colour_has_no_name() {
        assert_eq!(presets().name_of(Rgb::new(0, 255, 0)), None);
    }

    #[test]
    fn the_latest_name_for_a_colour_wins() {
        let mut presets = presets();
        presets.items.push(Preset {
            name: "Ink".to_string(),
            colour: Rgb::new(0, 0, 0),
        });

        assert_eq!(presets.name_of(Rgb::new(0, 0, 0)), Some("Ink"));
    }
}

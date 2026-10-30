//! Settings that outlive the window.
//!
//! Everything the player sets is written to `config.txt` next to the executable
//! as soon as it changes, so the next start looks like the last one.

use std::fs;
use std::path::PathBuf;

use dyetrace_core::image::Rgb;

use crate::hotkey;

const FILE_NAME: &str = "config.txt";

/// Where the settings live: next to the executable.
pub fn data_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()))
        .unwrap_or_default()
}

fn path() -> PathBuf {
    data_dir().join(FILE_NAME)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub colour: Rgb,
    pub spots: usize,
    pub retries: usize,
    pub variance: u8,
    /// Chosen samples, 0-based as the dye window numbers them.
    pub slots: Vec<usize>,
    /// Key id of the stop key (the Win32 virtual-key number; the X11
    /// backend maps it to a keysym).
    pub stop_key: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            colour: Rgb::black(),
            spots: 1,
            retries: 20,
            variance: 0,
            slots: Vec::new(),
            stop_key: hotkey::DEFAULT_KEY,
        }
    }
}

impl Config {
    /// The saved settings, with anything missing or unreadable left at its
    /// default.
    pub fn load() -> Self {
        match fs::read_to_string(path()) {
            Ok(text) => Self::parse(&text),
            Err(_) => Self::default(),
        }
    }

    /// Read the settings out of a config file, leaving anything missing or
    /// unreadable at its default.
    pub fn parse(text: &str) -> Self {
        let mut config = Self::default();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();

            match key.trim() {
                "colour" => {
                    if let Ok(hex) = u32::from_str_radix(value.trim_start_matches('#'), 16) {
                        config.colour = Rgb::from_u32(hex);
                    }
                }
                "spots" => {
                    if let Ok(spots) = value.parse::<usize>() {
                        config.spots = spots.clamp(1, dyetrace_core::MAX_TARGETS);
                    }
                }
                "retries" => {
                    if let Ok(retries) = value.parse::<usize>() {
                        config.retries = retries.min(99);
                    }
                }
                "variance" => {
                    if let Ok(variance) = value.parse::<u8>() {
                        config.variance = variance;
                    }
                }
                "slots" => {
                    config.slots = value
                        .split(',')
                        .filter_map(|slot| slot.trim().parse::<usize>().ok())
                        .filter(|slot| *slot < dyetrace_core::MAX_TARGETS)
                        .collect();
                    config.slots.sort_unstable();
                    config.slots.dedup();
                }
                "stop_key" => {
                    if let Ok(key) = value.parse::<i32>() {
                        if hotkey::key_for(key).is_some() {
                            config.stop_key = key;
                        }
                    }
                }
                _ => {}
            }
        }

        config
    }

    pub fn save(&self) {
        let _ = fs::write(path(), self.render());
    }

    /// The file contents for these settings.
    pub fn render(&self) -> String {
        let slots: Vec<String> = self.slots.iter().map(|slot| slot.to_string()).collect();
        format!(
            "# DyeTrace settings\ncolour={:02X}{:02X}{:02X}\nspots={}\nretries={}\nvariance={}\nslots={}\nstop_key={}\n",
            self.colour.r,
            self.colour.g,
            self.colour.b,
            self.spots,
            self.retries,
            self.variance,
            slots.join(","),
            self.stop_key,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Config {
        Config {
            colour: Rgb::new(0x12, 0x34, 0x56),
            spots: 3,
            retries: 7,
            variance: 9,
            slots: vec![0, 2],
            stop_key: 0x71,
        }
    }

    #[test]
    fn what_is_written_is_read_back() {
        let saved = settings();
        assert_eq!(Config::parse(&saved.render()), saved);
    }

    #[test]
    fn a_missing_file_is_all_defaults() {
        assert_eq!(Config::parse(""), Config::default());
    }

    #[test]
    fn nonsense_values_are_ignored() {
        let parsed = Config::parse("spots=nine\nvariance=999\nstop_key=1\ncolour=zzz\n");
        let defaults = Config::default();
        assert_eq!(parsed.spots, defaults.spots);
        assert_eq!(parsed.variance, defaults.variance);
        assert_eq!(parsed.colour, defaults.colour);
        assert_eq!(parsed.stop_key, defaults.stop_key, "only keys we can name are kept");
    }

    #[test]
    fn slots_are_cleaned_up() {
        let parsed = Config::parse("slots=4,1,1,9\n");
        assert_eq!(parsed.slots, vec![1, 4]);
    }
}

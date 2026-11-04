//! Settings kept between runs, in `config.json` in the data folder.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;

pub const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// The ISBN database (CSV file) used to fill in new books.
    #[serde(default)]
    pub isbn_database: Option<PathBuf>,
}

impl Config {
    /// A missing or unreadable file gives the defaults: settings must never
    /// keep Biblo from starting.
    pub fn load(dir: &Path) -> Config {
        fs::read_to_string(dir.join(CONFIG_FILE))
            .ok()
            .and_then(|text| serde_json::from_str(text.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> Result<(), Error> {
        let io = |e: std::io::Error| Error::Io(e.to_string());
        let json = serde_json::to_string_pretty(self).map_err(|e| Error::Json(e.to_string()))?;
        fs::create_dir_all(dir).map_err(io)?;
        let tmp = dir.join(format!("{CONFIG_FILE}.tmp"));
        fs::write(&tmp, json).map_err(io)?;
        fs::rename(&tmp, dir.join(CONFIG_FILE)).map_err(io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_a_save_and_bad_files_give_defaults() {
        let dir = std::env::temp_dir().join(format!("biblo-config-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(Config::load(&dir), Config::default());
        let config = Config {
            isbn_database: Some(PathBuf::from("C:\\data\\isbn.csv")),
        };
        config.save(&dir).unwrap();
        assert_eq!(Config::load(&dir), config);
        fs::write(dir.join(CONFIG_FILE), "{ cassé").unwrap();
        assert_eq!(Config::load(&dir), Config::default());
        let _ = fs::remove_dir_all(&dir);
    }
}

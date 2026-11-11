//! User settings, read from `<config dir>/coda-writer/config.toml`
//! (`~/.config/coda-writer/config.toml` on Linux). The file is optional.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Folder that holds the notes. `~` expands to the home directory.
    pub notes_dir: Option<String>,
    /// Blog server to publish to. Publishing is unavailable without it.
    pub server: Option<crate::publish::Server>,
}

impl Config {
    pub fn load(config_dir: &Path) -> Self {
        let file = config_dir.join("coda-writer").join("config.toml");
        match std::fs::read_to_string(&file) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
                eprintln!("coda-writer: ignoring {}: {e}", file.display());
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// `CODA_NOTES_DIR` wins over the config file, which wins over `~/Notes`.
    pub fn notes_dir(&self, home: &Path) -> PathBuf {
        let configured = std::env::var("CODA_NOTES_DIR").ok().or_else(|| self.notes_dir.clone());
        match configured.as_deref().map(str::trim).filter(|dir| !dir.is_empty()) {
            Some("~") => home.to_path_buf(),
            Some(dir) => match dir.strip_prefix("~/") {
                Some(rest) => home.join(rest),
                None => PathBuf::from(dir),
            },
            None => home.join("Notes"),
        }
    }
}

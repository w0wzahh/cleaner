//! Persistent user preferences for Cleaner.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::themes::Theme;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    #[serde(default)]
    pub theme: Theme,
    pub use_trash: bool,
    pub dry_run: bool,
    pub recursive: bool,
    pub include_hidden: bool,
    pub confirm_clean: bool,
    #[serde(default)]
    pub secure_delete: bool,
    pub default_dir: String,
    pub github_url: String,
    pub log_file: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            use_trash: true,
            dry_run: true,
            recursive: false,
            include_hidden: false,
            confirm_clean: true,
            secure_delete: false,
            default_dir: home_default_dir(),
            github_url: "https://github.com/w0wzahh".to_string(),
            log_file: "cleaner_history.log".to_string(),
        }
    }
}

impl Settings {
    /// Prefer loading next to the running binary, falling back to CWD and then defaults.
    pub fn load() -> Self {
        if let Some(path) = binary_settings_path() {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(s) = serde_json::from_str(&data) {
                    return s;
                }
            }
        }
        if let Ok(data) = std::fs::read_to_string("cleaner_settings.json") {
            if let Ok(s) = serde_json::from_str(&data) {
                return s;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = binary_settings_path().unwrap_or_else(|| PathBuf::from("cleaner_settings.json"));
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// Path used for the persistent operation log. Defaults next to the binary.
    pub fn log_path(&self) -> PathBuf {
        if !self.log_file.is_empty() {
            PathBuf::from(&self.log_file)
        } else {
            binary_dir().unwrap_or_else(|| PathBuf::from(".")).join("cleaner_history.log")
        }
    }
}

// -----------------------------------------------------------------------------
// helpers
// -----------------------------------------------------------------------------

fn home_default_dir() -> String {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .display()
        .to_string()
}

/// Try to resolve a location next to the running executable.
pub(crate) fn binary_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()))
}

pub(crate) fn default_dir() -> String {
    binary_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .display()
        .to_string()
}

fn binary_settings_path() -> Option<PathBuf> {
    binary_dir().map(|d| d.join("cleaner_settings.json"))
}

/// Where the settings file lives (or will be written) — used by the About tab.
pub fn settings_file_path() -> PathBuf {
    binary_settings_path().unwrap_or_else(|| PathBuf::from("cleaner_settings.json"))
}

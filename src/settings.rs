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
    /// User-defined System Cleaner targets.
    #[serde(default)]
    pub custom_targets: Vec<crate::models::CustomTarget>,
    /// Paths that are never deleted — one per line or comma-separated.
    #[serde(default)]
    pub protected_paths: String,
    /// Lifetime counters shown on the dashboard.
    #[serde(default)]
    pub total_files_cleaned: u64,
    #[serde(default)]
    pub total_space_freed: u64,
    /// Files cleaned per day ("YYYY-MM-DD" -> count) for the dashboard chart.
    #[serde(default)]
    pub clean_history: std::collections::BTreeMap<String, u64>,
    /// Timestamp of the last real clean, for display.
    #[serde(default)]
    pub last_clean: String,
    /// Scheduled scans — run automatically every N hours while the app is open.
    #[serde(default)]
    pub schedule_enabled: bool,
    #[serde(default = "default_schedule_hours")]
    pub schedule_hours: u32,
    #[serde(default)]
    pub schedule_target: ScheduleTarget,
    /// If true, a scheduled scan is followed by a real clean (honors dry run —
    /// with dry run on it only previews).
    #[serde(default)]
    pub schedule_auto_clean: bool,
    /// Unix timestamp of the last scheduled run (0 = never).
    #[serde(default)]
    pub schedule_last_run: i64,
}

fn default_schedule_hours() -> u32 {
    24
}

/// What a scheduled scan operates on.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScheduleTarget {
    System,
    Custom,
}

impl Default for ScheduleTarget {
    fn default() -> Self {
        Self::System
    }
}

impl ScheduleTarget {
    pub fn label(&self) -> &'static str {
        match self {
            Self::System => "System junk",
            Self::Custom => "Custom folder",
        }
    }

    pub fn all() -> &'static [ScheduleTarget] {
        &[Self::System, Self::Custom]
    }
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
            custom_targets: Vec::new(),
            protected_paths: String::new(),
            total_files_cleaned: 0,
            total_space_freed: 0,
            clean_history: std::collections::BTreeMap::new(),
            last_clean: String::new(),
            schedule_enabled: false,
            schedule_hours: 24,
            schedule_target: ScheduleTarget::System,
            schedule_auto_clean: false,
            schedule_last_run: 0,
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

    /// Protected paths as a list, split on commas and newlines.
    pub fn protected_list(&self) -> Vec<String> {
        self.protected_paths
            .split(|c| c == ',' || c == '\n')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
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

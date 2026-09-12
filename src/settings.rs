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
    /// Whether the getting-started card on the dashboard has been dismissed.
    #[serde(default)]
    pub welcomed: bool,
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
            welcomed: false,
        }
    }
}

impl Settings {
    /// Load from the data dir first, then migrate-friendly fallbacks:
    /// the old next-to-exe location, then CWD, then defaults.
    pub fn load() -> Self {
        let mut candidates: Vec<PathBuf> = vec![data_dir().join("cleaner_settings.json")];
        if let Some(exe_dir) = binary_dir() {
            candidates.push(exe_dir.join("cleaner_settings.json"));
        }
        candidates.push(PathBuf::from("cleaner_settings.json"));
        for path in candidates {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(s) = serde_json::from_str(&data) {
                    return s;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = data_dir().join("cleaner_settings.json");
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// Path used for the persistent operation log. Relative names resolve
    /// inside the data dir; absolute paths are honored as-is.
    pub fn log_path(&self) -> PathBuf {
        if !self.log_file.is_empty() {
            let p = PathBuf::from(&self.log_file);
            if p.is_absolute() {
                p
            } else {
                data_dir().join(p)
            }
        } else {
            data_dir().join("cleaner_history.log")
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

/// Where the app keeps its data (settings, history log, exported reports).
///
/// Two modes:
/// - **Portable / dev** — if the exe's folder is writable, everything stays
///   next to the binary, exactly like before.
/// - **Installed** — when the exe sits somewhere read-only (e.g.
///   `Program Files` after using the installer), data goes to
///   `%APPDATA%\Cleaner\` instead.
///
/// The folder gets a `README.txt` explaining what's inside.
pub fn data_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let dir = if let Some(exe_dir) = binary_dir() {
            if dir_writable(&exe_dir) {
                exe_dir
            } else {
                let base =
                    dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
                base.join("Cleaner")
            }
        } else {
            PathBuf::from(".")
        };
        let _ = std::fs::create_dir_all(&dir);
        write_data_readme(&dir);
        dir
    })
    .clone()
}

/// Folder for exported reports — `reports\` inside the data dir.
pub fn reports_dir() -> PathBuf {
    let dir = data_dir().join("reports");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// True if we can create files in `dir` (probes by writing a temp file).
fn dir_writable(dir: &std::path::Path) -> bool {
    let probe = dir.join(".cleaner_write_probe");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Drop a plain-language guide into the data folder so curious users can tell
/// what every file is for. Written once; never overwrites a user's copy.
fn write_data_readme(dir: &std::path::Path) {
    let path = dir.join("README.txt");
    if path.exists() {
        return;
    }
    let _ = std::fs::write(
        &path,
        "Cleaner — data folder\r\n\
         =====================\r\n\
         \r\n\
         Everything the app saves lives in this folder. Nothing is sent\r\n\
         anywhere — there is no telemetry, no accounts, no network calls.\r\n\
         \r\n\
         What each file is:\r\n\
         \r\n\
           cleaner_settings.json   All settings, stats, and your schedule.\r\n\
                                   Plain JSON — you can read or edit it.\r\n\
         \r\n\
           cleaner_history.log     A running log of every scan and clean,\r\n\
                                   so you can see exactly what happened.\r\n\
         \r\n\
           reports\\                Exported scan reports (.txt), one per\r\n\
                                   export, timestamped.\r\n\
         \r\n\
           README.txt              This file.\r\n\
         \r\n\
         Want a completely clean slate? Close the app and delete this whole\r\n\
         folder — Cleaner recreates it with fresh defaults next launch.\r\n\
         (To remove the program itself, use Windows Settings > Apps.)\r\n",
    );
}

/// Where the settings file lives (or will be written) — used by the About tab.
pub fn settings_file_path() -> PathBuf {
    data_dir().join("cleaner_settings.json")
}

//! Shared data types and UI enums for Cleaner.
//!
//! This module is deliberately a "bag of types" so the other modules can import
//! exactly what they need without circular dependencies.

pub const CHANGELOG: &str = r#"
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.4.1] - 2026-09-12
### Added
- Sidebar navigation, card-based layout, per-theme accent colors.
- Large Files: selectable results with clean support.
- Dashboard stat cards, safety toggles, and recent activity view.
- Storage tab caches disk list and adds manual refresh.
### Fixed
- Crash after cleaning (summary string parsing panic).
- Cancel now works for custom, large file, system, and empty folder scans.
- Quick duplicate hash now only reads the first 8 KB (was reading whole files).
- Zero-byte files no longer reported as duplicates.
- Empty folder removal now respects dry run and recycle bin settings.
- Duplicate "wasted space" total now tracks checkbox selection.
- `default_dir` setting is now actually honored.

## [2.4.0] - 2025-02-01
### Added
- Five selectable themes: Dark, Light, Nord, Dracula, Solarized.
- Animated theme transitions (0.35s ease-out interpolation).
- Empty Folder cleaner tab (bottom-up cascade detection).
- Secure Delete option (3-pass overwrite with random data before removal).
- "Export Report" button on Custom, Duplicates, Large Files, and Empty Folders tabs.
- Procedurally-generated app icon (sparkle on gradient circle).
### Changed
- Theme selector now lives in the top toolbar as a ComboBox.
- Settings schema updated (auto-resets on incompatible load).
- Centralized confirmation dialog via a `ConfirmAction` enum.
### Fixed
- Version string now consistently reflects Cargo.toml.

## [2.3.0] - 2025-01-25
### Added
- "Check for Updates" button in About tab (opens GitHub releases page).
- Unhinged comments everywhere.
### Changed
- Version bumped to 2.3.0.

## [2.2.0] - 2025-01-20
### Added
- Spinner animation in status bar.
- Two-stage duplicate scanning.
### Changed
- Version bumped to 2.2.0.

## [2.1.0] - 2025-01-15
### Added
- Storage Overview tab.
- Changelog tab.
- GitHub link in About.
- Persistent history log.

## [2.0.0] - 2025-01-10
### Added
- Initial release with full GUI.
"#;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Tab {
    Dashboard,
    CustomClean,
    Duplicates,
    LargeFiles,
    SystemCleaner,
    EmptyFolders,
    Storage,
    Changelog,
    About,
}

impl Tab {
    pub fn title(&self) -> &'static str {
        match self {
            Tab::Dashboard => "Dashboard",
            Tab::CustomClean => "Custom File Cleaner",
            Tab::Duplicates => "Duplicate File Finder",
            Tab::LargeFiles => "Large File Finder",
            Tab::SystemCleaner => "System Cleaner",
            Tab::EmptyFolders => "Empty Folder Cleaner",
            Tab::Storage => "Storage Overview",
            Tab::Changelog => "Changelog",
            Tab::About => "About",
        }
    }

    pub fn subtitle(&self) -> &'static str {
        match self {
            Tab::Dashboard => "Overview of your cleaning activity",
            Tab::CustomClean => "Scan a folder with filters, preview, then clean",
            Tab::Duplicates => "Two-stage hashing finds identical files fast",
            Tab::LargeFiles => "Hunt down the biggest space hogs",
            Tab::SystemCleaner => "Clear temp files and caches",
            Tab::EmptyFolders => "Remove empty directories, including cascades",
            Tab::Storage => "Per-drive usage at a glance (read-only)",
            Tab::Changelog => "What's new in Cleaner",
            Tab::About => "Version, links, and files",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ConfirmAction {
    CleanFiles,
    CleanDuplicates,
    CleanEmptyFolders,
}

#[derive(Clone, Debug)]
pub struct MatchedFile {
    pub path: std::path::PathBuf,
    pub size: u64,
}

#[derive(Clone, Debug)]
pub struct DuplicateGroup {
    pub hash: String,
    pub files: Vec<std::path::PathBuf>,
    pub size: u64,
}

#[derive(Clone, Debug)]
pub struct LargeFile {
    pub path: std::path::PathBuf,
    pub size: u64,
}

#[derive(Clone, Debug)]
pub struct SystemCleanTarget {
    pub name: String,
    pub path: std::path::PathBuf,
    pub description: String,
    pub enabled: bool,
}

#[derive(Clone, Debug)]
pub struct CustomCleanerState {
    pub dir_path: String,
    pub extensions: String,
    pub older_than_days: u64,
    pub min_size_bytes: u64,
    pub max_size_bytes: u64,
    pub pattern: String,
    pub exclude_dirs: String,
    pub matched_files: Vec<MatchedFile>,
    pub total_matched_size: u64,
}

impl Default for CustomCleanerState {
    fn default() -> Self {
        Self {
            dir_path: crate::settings::default_dir().to_string(),
            extensions: "tmp,log".to_string(),
            older_than_days: 0,
            min_size_bytes: 0,
            max_size_bytes: 0,
            pattern: String::new(),
            exclude_dirs: String::new(),
            matched_files: Vec::new(),
            total_matched_size: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DuplicateState {
    pub dir_path: String,
    pub groups: Vec<DuplicateGroup>,
    pub selected_files: Vec<std::path::PathBuf>,
    pub total_wasted: u64,
}

impl Default for DuplicateState {
    fn default() -> Self {
        Self {
            dir_path: crate::settings::default_dir().to_string(),
            groups: Vec::new(),
            selected_files: Vec::new(),
            total_wasted: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LargeFilesState {
    pub dir_path: String,
    pub threshold_mb: u64,
    pub files: Vec<LargeFile>,
    pub selected: Vec<std::path::PathBuf>,
    pub total_size: u64,
}

impl Default for LargeFilesState {
    fn default() -> Self {
        Self {
            dir_path: crate::settings::default_dir().to_string(),
            threshold_mb: 100,
            files: Vec::new(),
            selected: Vec::new(),
            total_size: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SystemCleanerState {
    pub targets: Vec<SystemCleanTarget>,
    pub matched_files: Vec<MatchedFile>,
    pub total_matched_size: u64,
}

impl Default for SystemCleanerState {
    fn default() -> Self {
        let mut targets = Vec::new();

        if let Some(dir) = dirs::cache_dir() {
            targets.push(SystemCleanTarget {
                name: "User Cache".to_string(),
                path: dir,
                description: "Cached data from applications".to_string(),
                enabled: true,
            });
        }
        if let Some(dir) = dirs::data_local_dir() {
            targets.push(SystemCleanTarget {
                name: "Local App Data Temp".to_string(),
                path: dir.join("Temp"),
                description: "Temporary files created by programs".to_string(),
                enabled: true,
            });
        }
        if let Some(dir) = dirs::data_dir() {
            targets.push(SystemCleanTarget {
                name: "App Data Temp".to_string(),
                path: dir.join("Temp"),
                description: "Temporary files".to_string(),
                enabled: false,
            });
        }
        if let Ok(temp) = std::env::var("TEMP") {
            targets.push(SystemCleanTarget {
                name: "System Temp".to_string(),
                path: std::path::PathBuf::from(temp),
                description: "System temporary files".to_string(),
                enabled: true,
            });
        }
        if let Some(home) = dirs::home_dir() {
            targets.push(SystemCleanTarget {
                name: "Chrome Cache".to_string(),
                path: home.join("AppData/Local/Google/Chrome/User Data/Default/Cache"),
                description: "Google Chrome browser cache".to_string(),
                enabled: false,
            });
            targets.push(SystemCleanTarget {
                name: "Edge Cache".to_string(),
                path: home.join("AppData/Local/Microsoft/Edge/User Data/Default/Cache"),
                description: "Microsoft Edge browser cache".to_string(),
                enabled: false,
            });
            targets.push(SystemCleanTarget {
                name: "Firefox Cache".to_string(),
                path: home.join("AppData/Local/Mozilla/Firefox/Profiles"),
                description: "Firefox browser cache (all profiles)".to_string(),
                enabled: false,
            });
            targets.push(SystemCleanTarget {
                name: "Thumbnail Cache".to_string(),
                path: home.join("AppData/Local/Microsoft/Windows/Explorer"),
                description: "Windows thumbnail cache".to_string(),
                enabled: false,
            });
        }

        Self {
            targets,
            matched_files: Vec::new(),
            total_matched_size: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EmptyFoldersState {
    pub dir_path: String,
    pub folders: Vec<std::path::PathBuf>,
}

impl Default for EmptyFoldersState {
    fn default() -> Self {
        Self {
            dir_path: crate::settings::default_dir().to_string(),
            folders: Vec::new(),
        }
    }
}

/// Snapshot of one mounted drive for the Storage tab.
#[derive(Clone, Debug)]
pub struct DiskEntry {
    pub name: String,
    pub mount: String,
    pub total: u64,
    pub available: u64,
}

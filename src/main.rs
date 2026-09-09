use eframe::egui;
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::Local;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;
use sysinfo::Disks;

// -----------------------------------------------------------------------------
// Embedded changelog (always available)
// -----------------------------------------------------------------------------
const CHANGELOG: &str = r#"
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.3.0] - 2025-01-25
### Added
- "Check for Updates" button in About tab (opens GitHub releases page)
- Unhinged comments everywhere – you're welcome.
### Changed
- Version bumped to 2.3.0 (for real this time).
### Fixed
- Compilation output now matches actual version.

## [2.2.0] - 2025-01-20
### Added
- Spinner animation in status bar during operations.
- Two-stage duplicate scanning: quick hash first, full hash only when necessary.
### Changed
- Version bumped to 2.2.0.
- Removed unused field `age_days` from MatchedFile.
- Improved performance of duplicate finder significantly.
### Fixed
- Compiler warnings (unused mut, dead code).

## [2.1.0] - 2025-01-15
### Added
- Storage Overview tab: shows disk usage per drive using sysinfo.
- Changelog tab: displays this embedded changelog.
- GitHub link field in Settings and About tab, openable in browser.
- Persistent history log: every action is appended to a log file (`cleaner_history.log` by default).
- Duplicate scanning now shows progress and is not stuck at 0%.
- New `open` crate dependency to open URLs.
### Changed
- Version bumped to 2.1.0.
- Improved UI: added more tabs, better spacing, and clearer sections.
- Duplicate scanning now sends progress updates during the hashing phase.
### Fixed
- Fixed duplicate scanning being stuck at 0% progress.
- Fixed unused variable warning in `poll_messages`.

## [2.0.0] - 2025-01-10
### Added
- Initial release of Cleaner with Dashboard, Custom Clean, Duplicates, Large Files, System Cleaner, and About tabs.
- Safe deletion to recycle bin (optional).
- Dry run mode and confirmation dialogs.
- Dark/light theme.
- Configuration persistence.
### Changed
- Complete rewrite of the previous simple file cleaner into a full-featured GUI application.
### Removed
- None (initial release).
"#;

// -----------------------------------------------------------------------------
// Settings (now includes GitHub URL and log file path)
// -----------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Clone)]
struct Settings {
    theme_dark: bool,
    use_trash: bool,
    dry_run: bool,
    recursive: bool,
    include_hidden: bool,
    confirm_clean: bool,
    default_dir: String,
    github_url: String,
    log_file: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_dark: true,
            use_trash: true,
            dry_run: true,
            recursive: false,
            include_hidden: false,
            confirm_clean: true,
            default_dir: dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).display().to_string(),
            github_url: "https://github.com/w0wzahh".to_string(),
            log_file: "cleaner_history.log".to_string(),
        }
    }
}

impl Settings {
    fn load() -> Self {
        if let Ok(data) = fs::read_to_string("cleaner_settings.json") {
            if let Ok(s) = serde_json::from_str(&data) {
                return s;
            }
        }
        Self::default()
    }
    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write("cleaner_settings.json", json);
        }
    }
}

// -----------------------------------------------------------------------------
// Tab definitions
// -----------------------------------------------------------------------------
#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Dashboard,
    CustomClean,
    Duplicates,
    LargeFiles,
    SystemCleaner,
    Storage,
    Changelog,
    About,
}

// -----------------------------------------------------------------------------
// Custom Cleaner State
// -----------------------------------------------------------------------------
struct CustomCleanerState {
    dir_path: String,
    extensions: String,
    older_than_days: u64,
    min_size_bytes: u64,
    max_size_bytes: u64,
    pattern: String,
    exclude_dirs: String,
    matched_files: Vec<MatchedFile>,
    total_matched_size: u64,
}

impl Default for CustomCleanerState {
    fn default() -> Self {
        Self {
            dir_path: Settings::default().default_dir,
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

// -----------------------------------------------------------------------------
// Duplicate Finder State
// -----------------------------------------------------------------------------
#[derive(Clone)]
struct DuplicateGroup {
    hash: String,
    files: Vec<PathBuf>,
    size: u64,
}

struct DuplicateState {
    dir_path: String,
    groups: Vec<DuplicateGroup>,
    selected_files: Vec<PathBuf>,
    total_wasted: u64,
}

impl Default for DuplicateState {
    fn default() -> Self {
        Self {
            dir_path: Settings::default().default_dir,
            groups: Vec::new(),
            selected_files: Vec::new(),
            total_wasted: 0,
        }
    }
}

// -----------------------------------------------------------------------------
// Large Files State
// -----------------------------------------------------------------------------
struct LargeFile {
    path: PathBuf,
    size: u64,
}

struct LargeFilesState {
    dir_path: String,
    threshold_mb: u64,
    files: Vec<LargeFile>,
    total_size: u64,
}

impl Default for LargeFilesState {
    fn default() -> Self {
        Self {
            dir_path: Settings::default().default_dir,
            threshold_mb: 100,
            files: Vec::new(),
            total_size: 0,
        }
    }
}

// -----------------------------------------------------------------------------
// System Cleaner State
// -----------------------------------------------------------------------------
#[derive(Clone)]
struct SystemCleanTarget {
    name: String,
    path: PathBuf,
    description: String,
    enabled: bool,
}

struct SystemCleanerState {
    targets: Vec<SystemCleanTarget>,
    matched_files: Vec<MatchedFile>,
    total_matched_size: u64,
}

impl Default for SystemCleanerState {
    fn default() -> Self {
        let mut targets = Vec::new();

        // User Cache – the place where apps stash their garbage
        if let Some(dir) = dirs::cache_dir() {
            targets.push(SystemCleanTarget {
                name: "User Cache".to_string(),
                path: dir,
                description: "Cached data from applications".to_string(),
                enabled: true,
            });
        }
        // Local App Data Temp – the digital dumpster behind the strip mall
        if let Some(dir) = dirs::data_local_dir() {
            targets.push(SystemCleanTarget {
                name: "Local App Data Temp".to_string(),
                path: dir.join("Temp"),
                description: "Temporary files created by programs".to_string(),
                enabled: true,
            });
        }
        // App Data Temp – sometimes programs forget to clean their room
        if let Some(dir) = dirs::data_dir() {
            targets.push(SystemCleanTarget {
                name: "App Data Temp".to_string(),
                path: dir.join("Temp"),
                description: "Temporary files".to_string(),
                enabled: false,
            });
        }
        // Windows Temp – the place where Windows puts things it doesn't want to talk about
        if let Ok(temp) = std::env::var("TEMP") {
            targets.push(SystemCleanTarget {
                name: "System Temp".to_string(),
                path: PathBuf::from(temp),
                description: "System temporary files".to_string(),
                enabled: true,
            });
        }
        // Browser caches – where your web history goes to die
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

// -----------------------------------------------------------------------------
// Matched File (generic)
// -----------------------------------------------------------------------------
#[derive(Clone)]
struct MatchedFile {
    path: PathBuf,
    size: u64,
}

// -----------------------------------------------------------------------------
// Worker messages
// -----------------------------------------------------------------------------
enum WorkerMessage {
    Log(String),
    Progress(f32),
    CustomMatched(Vec<MatchedFile>),
    Duplicates(Vec<DuplicateGroup>),
    LargeFiles(Vec<LargeFile>),
    SystemMatched(Vec<MatchedFile>),
    Done { summary: String },
    Error(String),
    Cancelled,
}

// -----------------------------------------------------------------------------
// Helper functions
// -----------------------------------------------------------------------------
fn file_age_days(path: &Path) -> Option<u64> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let duration = modified.duration_since(UNIX_EPOCH).ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    Some(now.as_secs().saturating_sub(duration.as_secs()) / 86_400)
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

fn collect_files(dir: &Path, recursive: bool, include_hidden: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if recursive && (include_hidden || !is_hidden(&path)) {
                    files.extend(collect_files(&path, recursive, include_hidden));
                }
            } else {
                if include_hidden || !is_hidden(&path) {
                    files.push(path);
                }
            }
        }
    }
    files
}

fn matches_glob(path: &Path, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    if let Ok(pat) = glob::Pattern::new(pattern) {
        pat.matches_path(path)
    } else {
        false
    }
}

fn is_excluded(path: &Path, exclude_dirs: &[String]) -> bool {
    for ex in exclude_dirs {
        if !ex.trim().is_empty() && path.starts_with(ex.trim()) {
            return true;
        }
    }
    false
}

fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

// -----------------------------------------------------------------------------
// Workers
// -----------------------------------------------------------------------------
fn custom_scan_worker(
    dir_path: String,
    extensions: String,
    older_than_days: u64,
    min_size_bytes: u64,
    max_size_bytes: u64,
    pattern: String,
    exclude_dirs: String,
    recursive: bool,
    include_hidden: bool,
    cancel_flag: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerMessage>,
) {
    let dir = PathBuf::from(&dir_path);
    if !dir.exists() {
        tx.send(WorkerMessage::Error(format!("Directory does not exist: {}", dir.display()))).unwrap();
        return;
    }
    if !dir.is_dir() {
        tx.send(WorkerMessage::Error(format!("Not a directory: {}", dir.display()))).unwrap();
        return;
    }

    let exts: Vec<String> = extensions.split(',')
        .map(|s| s.trim().trim_start_matches('.').to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    let exclude: Vec<String> = exclude_dirs.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    tx.send(WorkerMessage::Log("Scanning files...".to_string())).unwrap();
    let files = collect_files(&dir, recursive, include_hidden);
    let total = files.len();
    tx.send(WorkerMessage::Log(format!("Found {} files", total))).unwrap();

    let mut matched = Vec::new();
    for (i, file) in files.iter().enumerate() {
        if cancel_flag.load(Ordering::Relaxed) {
            tx.send(WorkerMessage::Cancelled).unwrap();
            return;
        }
        if i % 10 == 0 || i == total - 1 {
            tx.send(WorkerMessage::Progress((i + 1) as f32 / total as f32)).unwrap();
        }
        if is_excluded(file, &exclude) { continue; }
        if !exts.is_empty() {
            let ext = file.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase());
            if !ext.map(|e| exts.contains(&e)).unwrap_or(false) { continue; }
        }
        if !pattern.is_empty() && !matches_glob(file, &pattern) { continue; }
        if older_than_days > 0 {
            if let Some(age) = file_age_days(file) {
                if age < older_than_days { continue; }
            } else { continue; }
        }
        if let Ok(meta) = fs::metadata(file) {
            let size = meta.len();
            if size < min_size_bytes { continue; }
            if max_size_bytes > 0 && size > max_size_bytes { continue; }
        }
        let size = fs::metadata(file).map(|m| m.len()).unwrap_or(0);
        matched.push(MatchedFile { path: file.clone(), size });
    }

    let matched_count = matched.len();
    let total_size: u64 = matched.iter().map(|f| f.size).sum();
    tx.send(WorkerMessage::Log(format!("Matched {} files, total {}", matched_count, human_size(total_size)))).unwrap();
    tx.send(WorkerMessage::CustomMatched(matched)).unwrap();
    tx.send(WorkerMessage::Done { summary: format!("Scan complete. {} files matched.", matched_count) }).unwrap();
}

fn duplicates_worker(
    dir_path: String,
    cancel_flag: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerMessage>,
) {
    let dir = PathBuf::from(&dir_path);
    if !dir.exists() || !dir.is_dir() {
        tx.send(WorkerMessage::Error(format!("Invalid directory: {}", dir.display()))).unwrap();
        return;
    }

    tx.send(WorkerMessage::Log("Scanning for duplicates (optimized)...".to_string())).unwrap();

    // First pass: group by file size
    let mut size_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut total_files = 0;
    for entry in WalkDir::new(&dir).follow_links(false) {
        if cancel_flag.load(Ordering::Relaxed) {
            tx.send(WorkerMessage::Cancelled).unwrap();
            return;
        }
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() {
                if let Ok(meta) = fs::metadata(path) {
                    size_map.entry(meta.len()).or_default().push(path.to_path_buf());
                    total_files += 1;
                }
            }
        }
    }

    tx.send(WorkerMessage::Log(format!("Found {} files, grouping by size...", total_files))).unwrap();
    tx.send(WorkerMessage::Progress(0.1)).unwrap();

    let mut groups = Vec::new();
    let mut processed_groups = 0;
    let total_groups = size_map.len();

    for (size, files) in size_map {
        if files.len() < 2 {
            processed_groups += 1;
            continue;
        }

        // Quick hash (first 8KB) to filter candidates racial profiling bitches
        let mut quick_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for file in &files {
            if cancel_flag.load(Ordering::Relaxed) {
                tx.send(WorkerMessage::Cancelled).unwrap();
                return;
            }
            let quick_hash = match fs::read(&file) {
                Ok(bytes) => {
                    let len = bytes.len().min(8192);
                    let mut hasher = Sha256::new();
                    hasher.update(&bytes[..len]);
                    hasher.update(&size.to_le_bytes()); // include size for extra safety
                    format!("{:x}", hasher.finalize())
                }
                Err(_) => continue,
            };
            quick_map.entry(quick_hash).or_default().push(file.clone());
        }

        // Full hash only for quick groups with >1 file – now we bring out the full forensics kit
        for (_, quick_files) in quick_map {
            if quick_files.len() < 2 { continue; }
            let mut full_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
            for file in &quick_files {
                if cancel_flag.load(Ordering::Relaxed) {
                    tx.send(WorkerMessage::Cancelled).unwrap();
                    return;
                }
                if let Ok(bytes) = fs::read(file) {
                    let hash = Sha256::digest(&bytes);
                    let hash_str = format!("{:x}", hash);
                    full_map.entry(hash_str).or_default().push(file.clone());
                }
            }
            for (hash, full_files) in full_map {
                if full_files.len() > 1 {
                    groups.push(DuplicateGroup {
                        hash,
                        files: full_files,
                        size,
                    });
                }
            }
        }

        processed_groups += 1;
        let progress = 0.1 + 0.85 * (processed_groups as f32 / total_groups.max(1) as f32);
        tx.send(WorkerMessage::Progress(progress)).unwrap();
        if processed_groups % 10 == 0 {
            tx.send(WorkerMessage::Log(format!("Processed {}/{} groups...", processed_groups, total_groups))).unwrap();
        }
    }

    let groups_len = groups.len();
    let total_wasted: u64 = groups.iter().map(|g| g.size * (g.files.len() as u64 - 1)).sum();
    tx.send(WorkerMessage::Progress(0.97)).unwrap();
    tx.send(WorkerMessage::Log(format!("Found {} duplicate groups, wasting {}", groups_len, human_size(total_wasted)))).unwrap();
    tx.send(WorkerMessage::Duplicates(groups)).unwrap();
    tx.send(WorkerMessage::Progress(1.0)).unwrap();
    tx.send(WorkerMessage::Done { summary: format!("Duplicate scan complete. Found {} groups.", groups_len) }).unwrap();
}

fn large_files_worker(
    dir_path: String,
    threshold_mb: u64,
    cancel_flag: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerMessage>,
) {
    let dir = PathBuf::from(&dir_path);
    if !dir.exists() || !dir.is_dir() {
        tx.send(WorkerMessage::Error(format!("Invalid directory: {}", dir.display()))).unwrap();
        return;
    }

    let threshold = threshold_mb * 1024 * 1024;
    tx.send(WorkerMessage::Log(format!("Scanning for files larger than {} MB...", threshold_mb))).unwrap();

    let mut large_files = Vec::new();
    for entry in WalkDir::new(&dir).follow_links(false) {
        if cancel_flag.load(Ordering::Relaxed) {
            tx.send(WorkerMessage::Cancelled).unwrap();
            return;
        }
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() {
                if let Ok(meta) = fs::metadata(path) {
                    if meta.len() > threshold {
                        large_files.push(LargeFile {
                            path: path.to_path_buf(),
                            size: meta.len(),
                        });
                    }
                }
            }
        }
    }

    large_files.sort_by(|a, b| b.size.cmp(&a.size));
    let total_size: u64 = large_files.iter().map(|f| f.size).sum();
    tx.send(WorkerMessage::Log(format!("Found {} large files, total {}", large_files.len(), human_size(total_size)))).unwrap();
    tx.send(WorkerMessage::LargeFiles(large_files)).unwrap();
    tx.send(WorkerMessage::Done { summary: format!("Large file scan complete.") }).unwrap();
}

fn system_scan_worker(
    targets: Vec<SystemCleanTarget>,
    cancel_flag: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerMessage>,
) {
    let mut matched = Vec::new();
    for target in targets {
        if !target.enabled { continue; }
        tx.send(WorkerMessage::Log(format!("Scanning {} ...", target.name))).unwrap();
        if let Ok(files) = fs::read_dir(&target.path) {
            for entry in files.flatten() {
                if cancel_flag.load(Ordering::Relaxed) {
                    tx.send(WorkerMessage::Cancelled).unwrap();
                    return;
                }
                let path = entry.path();
                if let Ok(meta) = fs::metadata(&path) {
                    if meta.is_file() {
                        let size = meta.len();
                        matched.push(MatchedFile { path, size });
                    }
                }
            }
        }
    }
    let total_size: u64 = matched.iter().map(|f| f.size).sum();
    tx.send(WorkerMessage::Log(format!("System scan found {} files, total {}", matched.len(), human_size(total_size)))).unwrap();
    tx.send(WorkerMessage::SystemMatched(matched)).unwrap();
    tx.send(WorkerMessage::Done { summary: format!("System scan complete.") }).unwrap();
}

fn clean_files(
    files: Vec<MatchedFile>,
    use_trash: bool,
    dry_run: bool,
    cancel_flag: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerMessage>,
) {
    let total = files.len();
    let mut deleted = 0;
    let mut errors = 0;
    let mut freed = 0u64;

    for (i, file) in files.iter().enumerate() {
        if cancel_flag.load(Ordering::Relaxed) {
            tx.send(WorkerMessage::Cancelled).unwrap();
            return;
        }
        tx.send(WorkerMessage::Progress((i + 1) as f32 / total as f32)).unwrap();
        if dry_run {
            tx.send(WorkerMessage::Log(format!("[DRY] Would delete: {}", file.path.display()))).unwrap();
            freed += file.size;
            deleted += 1;
            continue;
        }
        let result: Result<(), String> = if use_trash {
            trash::delete(&file.path).map_err(|e| e.to_string())
        } else {
            fs::remove_file(&file.path).map_err(|e| e.to_string())
        };
        match result {
            Ok(_) => {
                deleted += 1;
                freed += file.size;
                tx.send(WorkerMessage::Log(format!("Deleted: {}", file.path.display()))).unwrap();
            }
            Err(e) => {
                errors += 1;
                tx.send(WorkerMessage::Log(format!("Error deleting {}: {}", file.path.display(), e))).unwrap();
            }
        }
    }
    let summary = if dry_run {
        format!("Dry run complete. Would delete {} files, freeing {}.", deleted, human_size(freed))
    } else {
        format!("Cleaning complete. Deleted {} files, freed {}, {} errors.", deleted, human_size(freed), errors)
    };
    tx.send(WorkerMessage::Done { summary }).unwrap();
}

// -----------------------------------------------------------------------------
// Main App
// -----------------------------------------------------------------------------
struct CleanerApp {
    tab: Tab,
    settings: Settings,
    scanning: bool,
    cleaning: bool,
    cancel_flag: Arc<AtomicBool>,
    progress: f32,
    log: Vec<String>,
    rx: Option<mpsc::Receiver<WorkerMessage>>,
    status: String,
    show_confirm_clean: bool,

    custom: CustomCleanerState,
    duplicates: DuplicateState,
    large_files: LargeFilesState,
    system: SystemCleanerState,

    total_files_cleaned: u64,
    total_space_freed: u64,
    last_scan_summary: String,
}

impl Default for CleanerApp {
    fn default() -> Self {
        Self {
            tab: Tab::Dashboard,
            settings: Settings::load(),
            scanning: false,
            cleaning: false,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            progress: 0.0,
            log: Vec::new(),
            rx: None,
            status: "Ready".to_string(),
            show_confirm_clean: false,
            custom: CustomCleanerState::default(),
            duplicates: DuplicateState::default(),
            large_files: LargeFilesState::default(),
            system: SystemCleanerState::default(),
            total_files_cleaned: 0,
            total_space_freed: 0,
            last_scan_summary: "No scan yet".to_string(),
        }
    }
}

impl CleanerApp {
    fn add_log(&mut self, msg: &str) {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        let line = format!("[{}] {}", timestamp, msg);
        self.log.push(line.clone());
        if self.log.len() > 1000 {
            self.log.remove(0);
        }
        if let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.settings.log_file)
        {
            use std::io::Write;
            let _ = writeln!(file, "{}", line);
        }
    }

    fn start_custom_scan(&mut self) {
        if self.scanning || self.cleaning { return; }
        self.settings.save();
        self.scanning = true;
        self.progress = 0.0;
        self.custom.matched_files.clear();
        self.custom.total_matched_size = 0;
        self.log.clear();
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let dir = self.custom.dir_path.clone();
        let ext = self.custom.extensions.clone();
        let days = self.custom.older_than_days;
        let min = self.custom.min_size_bytes;
        let max = self.custom.max_size_bytes;
        let pat = self.custom.pattern.clone();
        let excl = self.custom.exclude_dirs.clone();
        let rec = self.settings.recursive;
        let hidden = self.settings.include_hidden;
        self.add_log("Starting custom scan...");
        thread::spawn(move || custom_scan_worker(dir, ext, days, min, max, pat, excl, rec, hidden, cancel, tx));
        self.rx = Some(rx);
        self.status = "Scanning...".to_string();
    }

    fn start_duplicates_scan(&mut self) {
        if self.scanning || self.cleaning { return; }
        self.settings.save();
        self.scanning = true;
        self.progress = 0.0;
        self.duplicates.groups.clear();
        self.duplicates.selected_files.clear();
        self.duplicates.total_wasted = 0;
        self.log.clear();
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let dir = self.duplicates.dir_path.clone();
        self.add_log("Starting duplicate scan...");
        thread::spawn(move || duplicates_worker(dir, cancel, tx));
        self.rx = Some(rx);
        self.status = "Scanning duplicates...".to_string();
    }

    fn start_large_files_scan(&mut self) {
        if self.scanning || self.cleaning { return; }
        self.settings.save();
        self.scanning = true;
        self.progress = 0.0;
        self.large_files.files.clear();
        self.large_files.total_size = 0;
        self.log.clear();
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let dir = self.large_files.dir_path.clone();
        let threshold = self.large_files.threshold_mb;
        self.add_log("Starting large file scan...");
        thread::spawn(move || large_files_worker(dir, threshold, cancel, tx));
        self.rx = Some(rx);
        self.status = "Scanning large files...".to_string();
    }

    fn start_system_scan(&mut self) {
        if self.scanning || self.cleaning { return; }
        self.settings.save();
        self.scanning = true;
        self.progress = 0.0;
        self.system.matched_files.clear();
        self.system.total_matched_size = 0;
        self.log.clear();
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let targets = self.system.targets.clone();
        self.add_log("Starting system scan...");
        thread::spawn(move || system_scan_worker(targets, cancel, tx));
        self.rx = Some(rx);
        self.status = "Scanning system...".to_string();
    }

    fn start_clean_selected(&mut self) {
        if self.scanning || self.cleaning { return; }
        let files: Vec<MatchedFile> = match self.tab {
            Tab::CustomClean => self.custom.matched_files.clone(),
            Tab::SystemCleaner => self.system.matched_files.clone(),
            _ => Vec::new(),
        };
        if files.is_empty() { return; }
        self.cleaning = true;
        self.progress = 0.0;
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let use_trash = self.settings.use_trash;
        let dry_run = self.settings.dry_run;
        self.add_log("Starting cleaning...");
        thread::spawn(move || clean_files(files, use_trash, dry_run, cancel, tx));
        self.rx = Some(rx);
        self.status = "Cleaning...".to_string();
    }

    fn start_clean_duplicates(&mut self) {
        if self.scanning || self.cleaning { return; }
        let files = self.duplicates.selected_files.clone();
        if files.is_empty() { return; }
        self.cleaning = true;
        self.progress = 0.0;
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let use_trash = self.settings.use_trash;
        let dry_run = self.settings.dry_run;
        let matched: Vec<MatchedFile> = files.into_iter().map(|p| {
            let size = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            MatchedFile { path: p, size }
        }).collect();
        self.add_log("Starting duplicate cleanup...");
        thread::spawn(move || clean_files(matched, use_trash, dry_run, cancel, tx));
        self.rx = Some(rx);
        self.status = "Cleaning duplicates...".to_string();
    }

    fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    fn poll_messages(&mut self, ctx: &egui::Context) {
        if let Some(rx) = self.rx.take() {
            let mut still_active = true;
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    WorkerMessage::Log(s) => self.add_log(&s),
                    WorkerMessage::Progress(p) => self.progress = p,
                    WorkerMessage::CustomMatched(files) => {
                        self.custom.total_matched_size = files.iter().map(|f| f.size).sum();
                        self.custom.matched_files = files;
                    }
                    WorkerMessage::Duplicates(groups) => {
                        let mut selected = Vec::new();
                        let mut wasted = 0;
                        for group in &groups {
                            if group.files.len() > 1 {
                                for file in group.files.iter().skip(1) {
                                    selected.push(file.clone());
                                    wasted += group.size;
                                }
                            }
                        }
                        self.duplicates.selected_files = selected;
                        self.duplicates.total_wasted = wasted;
                        self.duplicates.groups = groups;
                    }
                    WorkerMessage::LargeFiles(files) => {
                        self.large_files.total_size = files.iter().map(|f| f.size).sum();
                        self.large_files.files = files;
                    }
                    WorkerMessage::SystemMatched(files) => {
                        self.system.total_matched_size = files.iter().map(|f| f.size).sum();
                        self.system.matched_files = files;
                    }
                    WorkerMessage::Done { summary } => {
                        self.add_log(&summary);
                        self.status = if self.scanning { "Scan complete".to_string() } else { "Clean complete".to_string() };
                        if self.scanning {
                            self.scanning = false;
                            self.last_scan_summary = summary.clone();
                        }
                        if self.cleaning {
                            self.cleaning = false;
                            if !self.settings.dry_run {
                                if let Some(pos) = summary.find("Deleted ") {
                                    if let Some(end) = summary.find(" files") {
                                        if let Ok(num) = summary[pos + 8..end].parse::<u64>() {
                                            self.total_files_cleaned += num;
                                        }
                                    }
                                }
                            }
                        }
                        still_active = false;
                    }
                    WorkerMessage::Error(e) => {
                        self.add_log(&format!("ERROR: {}", e));
                        self.status = "Error".to_string();
                        self.scanning = false;
                        self.cleaning = false;
                        still_active = false;
                    }
                    WorkerMessage::Cancelled => {
                        self.add_log("Operation cancelled by user.");
                        self.status = "Cancelled".to_string();
                        self.scanning = false;
                        self.cleaning = false;
                        still_active = false;
                    }
                }
            }
            if still_active {
                self.rx = Some(rx);
                ctx.request_repaint();
            } else {
                self.rx = None;
            }
        }
    }

    // Static helper to draw a file list
    fn draw_file_list(ui: &mut egui::Ui, files: &[MatchedFile], id_source: &str) {
        egui::ScrollArea::vertical()
            .id_source(id_source)
            .max_height(ui.available_height() * 0.6)
            .show(ui, |ui| {
                for f in files {
                    ui.horizontal(|ui| {
                        ui.monospace(f.path.display().to_string());
                        ui.label(format!("({})", human_size(f.size)));
                    });
                }
                if files.is_empty() {
                    ui.label("No files to display.");
                }
            });
    }

    fn draw_duplicate_groups(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_source("duplicates_scroll")
            .max_height(ui.available_height() * 0.7)
            .show(ui, |ui| {
                for group in &self.duplicates.groups {
                    ui.collapsing(
                        format!("{} files, {} each (hash {})",
                            group.files.len(),
                            human_size(group.size),
                            &group.hash[..8]
                        ),
                        |ui| {
                            for (i, file) in group.files.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    if i > 0 {
                                        let mut selected = self.duplicates.selected_files.contains(file);
                                        if ui.checkbox(&mut selected, "").changed() {
                                            if selected {
                                                if !self.duplicates.selected_files.contains(file) {
                                                    self.duplicates.selected_files.push(file.clone());
                                                }
                                            } else {
                                                self.duplicates.selected_files.retain(|x| x != file);
                                            }
                                            self.duplicates.total_wasted = self.duplicates.groups.iter()
                                                .map(|g| g.size * (g.files.len() as u64 - 1))
                                                .sum();
                                        }
                                    } else {
                                        ui.label("✔");
                                    }
                                    ui.monospace(file.display().to_string());
                                });
                            }
                        }
                    );
                }
                if self.duplicates.groups.is_empty() {
                    ui.label("No duplicate groups found.");
                }
            });
    }

    fn draw_storage(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.heading("Storage Overview");
        ui.separator();

        let disks = Disks::new_with_refreshed_list();
        for disk in disks.list() {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total - available;
            let percent = if total > 0 { (used as f64 / total as f64) * 100.0 } else { 0.0 };

            ui.group(|ui| {
                ui.label(format!("Drive {} ({})", disk.name().to_string_lossy(), disk.mount_point().display()));
                ui.add(egui::ProgressBar::new((percent / 100.0) as f32).show_percentage().text(format!("{:.0}% used", percent)));
                ui.label(format!("Used: {} / {}", human_size(used), human_size(total)));
                ui.label(format!("Free: {}", human_size(available)));
            });
            ui.add_space(8.0);
        }

        ui.separator();
        ui.label("Note: This view is read-only and does not perform any cleaning.");
    }
}

// -----------------------------------------------------------------------------
// eframe App
// -----------------------------------------------------------------------------
impl eframe::App for CleanerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(if self.settings.theme_dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });

        self.poll_messages(ctx);

        // Top toolbar
        egui::TopBottomPanel::top("top_toolbar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("Cleaner");
                ui.separator();
                ui.selectable_value(&mut self.tab, Tab::Dashboard, "Dashboard");
                ui.selectable_value(&mut self.tab, Tab::CustomClean, "Custom Clean");
                ui.selectable_value(&mut self.tab, Tab::Duplicates, "Duplicates");
                ui.selectable_value(&mut self.tab, Tab::LargeFiles, "Large Files");
                ui.selectable_value(&mut self.tab, Tab::SystemCleaner, "System Cleaner");
                ui.selectable_value(&mut self.tab, Tab::Storage, "Storage");
                ui.selectable_value(&mut self.tab, Tab::Changelog, "Changelog");
                ui.selectable_value(&mut self.tab, Tab::About, "About");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut self.settings.theme_dark, "Dark theme");
                    if ui.button("Save settings").clicked() {
                        self.settings.save();
                        self.add_log("Settings saved.");
                    }
                });
            });
            ui.add_space(4.0);
        });

        // Bottom status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(format!("Status: {}", self.status));
                ui.separator();
                if self.scanning || self.cleaning {
                    ui.spinner();
                    ui.add(egui::ProgressBar::new(self.progress).show_percentage());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        self.cancel();
                    }
                });
            });
            ui.add_space(4.0);
        });

        // Main content
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.tab {
                Tab::Dashboard => self.draw_dashboard(ui),
                Tab::CustomClean => self.draw_custom_clean(ui),
                Tab::Duplicates => self.draw_duplicates(ui),
                Tab::LargeFiles => self.draw_large_files(ui),
                Tab::SystemCleaner => self.draw_system_cleaner(ui),
                Tab::Storage => self.draw_storage(ui),
                Tab::Changelog => self.draw_changelog(ui),
                Tab::About => self.draw_about(ui),
            }
        });

        // Confirmation dialog
        if self.show_confirm_clean {
            egui::Window::new("Confirm Clean")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Are you sure you want to proceed?");
                    ui.horizontal(|ui| {
                        if ui.button("Yes").clicked() {
                            self.show_confirm_clean = false;
                            self.start_clean_selected();
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_confirm_clean = false;
                        }
                    });
                });
        }
    }
}

// -----------------------------------------------------------------------------
// Drawing functions for each tab
// -----------------------------------------------------------------------------
impl CleanerApp {
    fn draw_dashboard(&mut self, ui: &mut egui::Ui) {
        ui.add_space(20.0);
        ui.heading("Dashboard");
        ui.separator();
        ui.columns(3, |cols| {
            cols[0].label(format!("Files cleaned: {}", self.total_files_cleaned));
            cols[1].label(format!("Space freed: {}", human_size(self.total_space_freed)));
            cols[2].label(format!("Last scan: {}", self.last_scan_summary));
        });
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            if ui.button("Quick Scan (Custom)").clicked() {
                self.tab = Tab::CustomClean;
                self.start_custom_scan();
            }
            if ui.button("Quick Scan (System)").clicked() {
                self.tab = Tab::SystemCleaner;
                self.start_system_scan();
            }
        });
        ui.add_space(20.0);
        ui.label("Welcome to Cleaner – your smart system maintenance tool.");
    }

    fn draw_custom_clean(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.heading("Custom File Cleaner");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Directory:");
            ui.text_edit_singleline(&mut self.custom.dir_path);
            if ui.button("Browse…").clicked() {
                if let Some(path) = FileDialog::new().pick_folder() {
                    self.custom.dir_path = path.display().to_string();
                }
            }
        });

        ui.collapsing("File Filters", |ui| {
            ui.label("Extensions (comma separated):");
            ui.text_edit_singleline(&mut self.custom.extensions);
            ui.label("Glob pattern (e.g. *.tmp):");
            ui.text_edit_singleline(&mut self.custom.pattern);
        });
        ui.collapsing("Age & Size", |ui| {
            ui.horizontal(|ui| {
                ui.label("Older than (days):");
                ui.add(egui::Slider::new(&mut self.custom.older_than_days, 0..=36500).text("days"));
            });
            ui.horizontal(|ui| {
                ui.label("Min size (bytes):");
                ui.add(egui::DragValue::new(&mut self.custom.min_size_bytes).speed(1000));
            });
            ui.horizontal(|ui| {
                ui.label("Max size (bytes, 0=∞):");
                ui.add(egui::DragValue::new(&mut self.custom.max_size_bytes).speed(1000));
            });
        });
        ui.collapsing("Advanced", |ui| {
            ui.label("Exclude directories (comma separated, absolute):");
            ui.text_edit_multiline(&mut self.custom.exclude_dirs);
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Scan").clicked() {
                self.start_custom_scan();
            }
            if ui.add_enabled(!self.custom.matched_files.is_empty(), egui::Button::new("Clean Selected")).clicked() {
                if self.settings.confirm_clean {
                    self.show_confirm_clean = true;
                } else {
                    self.start_clean_selected();
                }
            }
        });

        ui.separator();
        ui.label(format!("Matched: {} files, {}", self.custom.matched_files.len(), human_size(self.custom.total_matched_size)));
        Self::draw_file_list(ui, &self.custom.matched_files, "custom_preview_scroll");
    }

    fn draw_duplicates(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.heading("Duplicate File Finder");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Directory:");
            ui.text_edit_singleline(&mut self.duplicates.dir_path);
            if ui.button("Browse…").clicked() {
                if let Some(path) = FileDialog::new().pick_folder() {
                    self.duplicates.dir_path = path.display().to_string();
                }
            }
        });

        ui.add_space(8.0);
        if ui.button("Scan for Duplicates").clicked() {
            self.start_duplicates_scan();
        }
        ui.add_space(8.0);

        ui.label(format!("Found {} duplicate groups, total waste: {}", self.duplicates.groups.len(), human_size(self.duplicates.total_wasted)));
        ui.separator();

        self.draw_duplicate_groups(ui);

        if ui.button("Clean Selected Duplicates").clicked() {
            if self.settings.confirm_clean {
                self.show_confirm_clean = true;
            } else {
                self.start_clean_duplicates();
            }
        }
    }

    fn draw_large_files(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.heading("Large File Finder");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Directory:");
            ui.text_edit_singleline(&mut self.large_files.dir_path);
            if ui.button("Browse…").clicked() {
                if let Some(path) = FileDialog::new().pick_folder() {
                    self.large_files.dir_path = path.display().to_string();
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("Threshold (MB):");
            ui.add(egui::DragValue::new(&mut self.large_files.threshold_mb).speed(1));
        });

        if ui.button("Scan for Large Files").clicked() {
            self.start_large_files_scan();
        }
        ui.add_space(8.0);

        ui.label(format!("Found {} large files, total {}", self.large_files.files.len(), human_size(self.large_files.total_size)));
        egui::ScrollArea::vertical()
            .id_source("large_files_scroll")
            .max_height(ui.available_height() * 0.7)
            .show(ui, |ui| {
                for file in &self.large_files.files {
                    ui.horizontal(|ui| {
                        ui.monospace(file.path.display().to_string());
                        ui.label(format!("({})", human_size(file.size)));
                    });
                }
            });
    }

    fn draw_system_cleaner(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.heading("System Cleaner");
        ui.separator();

        ui.collapsing("Select cleaning targets", |ui| {
            for target in &mut self.system.targets {
                ui.checkbox(&mut target.enabled, &target.name);
                ui.label(&target.description);
                ui.monospace(format!("Path: {}", target.path.display()));
                ui.separator();
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Scan System").clicked() {
                self.start_system_scan();
            }
            if ui.add_enabled(!self.system.matched_files.is_empty(), egui::Button::new("Clean System")).clicked() {
                if self.settings.confirm_clean {
                    self.show_confirm_clean = true;
                } else {
                    self.start_clean_selected();
                }
            }
        });

        ui.add_space(8.0);
        ui.label(format!("Matched files: {}", self.system.matched_files.len()));
        Self::draw_file_list(ui, &self.system.matched_files, "system_scroll");
    }

    fn draw_changelog(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.heading("Changelog");
        ui.separator();
        ui.monospace(CHANGELOG);
    }

    fn draw_about(&mut self, ui: &mut egui::Ui) {
        ui.add_space(20.0);
        ui.heading("Cleaner");
        ui.label("Version 2.3.0");
        ui.label("A modern, fast, and safe system cleaning utility.");
        ui.add_space(10.0);
        ui.label("Features:");
        ui.label("• Custom file cleaning with filters");
        ui.label("• Duplicate file finder (lightning fast)");
        ui.label("• Large file finder");
        ui.label("• System junk cleaner (temp files, caches)");
        ui.label("• Safe deletion to recycle bin by default");
        ui.label("• Dark/light theme");
        ui.label("• Storage overview");
        ui.label("• Detailed changelog");
        ui.add_space(10.0);
        ui.label("GitHub repository:");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.settings.github_url);
            if ui.button("Open").clicked() {
                let _ = open::that(&self.settings.github_url);
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("Check for Updates").clicked() {
                // Open the releases page – user can download the latest version manually
                let _ = open::that("https://github.com/w0wzahh/cleaner/releases");
            }
        });
        ui.label("This application is written in Rust using egui.");
    }
}

// -----------------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------------
fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Cleaner",
        options,
        Box::new(|_cc| Box::new(CleanerApp::default())),
    )
}
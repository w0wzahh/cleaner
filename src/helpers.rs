//! Pure-ish helpers, file shredding, and report export.

use std::fs::{self, File};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Local;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::models::{LargeFile, MatchedFile};

// -----------------------------------------------------------------------------
// filesystem helpers
// -----------------------------------------------------------------------------

pub fn file_age_days(path: &Path) -> Option<u64> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let duration = modified.duration_since(UNIX_EPOCH).ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    Some(now.as_secs().saturating_sub(duration.as_secs()) / 86_400)
}

/// On Windows this checks the FILE_ATTRIBUTE_HIDDEN flag; elsewhere it
/// falls back to the dot-prefix convention.
pub fn is_hidden(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(meta) = fs::metadata(path) {
            const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
            if meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0 {
                return true;
            }
        }
    }
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// Iterative file collection — no recursion-depth risk, cancellable, and it
/// prunes excluded/hidden directories during traversal instead of after.
pub fn collect_files(
    dir: &Path,
    recursive: bool,
    include_hidden: bool,
    exclude: &[String],
    cancel_flag: &AtomicBool,
) -> Option<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut walker = WalkDir::new(dir).follow_links(false);
    if !recursive {
        walker = walker.max_depth(1);
    }
    for entry in walker.into_iter().filter_entry(|e| {
        let p = e.path();
        (include_hidden || !is_hidden(p)) && !is_excluded(p, exclude)
    }) {
        if cancel_flag.load(Ordering::Relaxed) {
            return None;
        }
        if let Ok(entry) = entry {
            if entry.path().is_file() {
                files.push(entry.path().to_path_buf());
            }
        }
    }
    Some(files)
}

/// Match a glob pattern. If the pattern contains no path separator it is
/// applied to the file name (the common case: `*.tmp`); otherwise it is
/// matched against the full path. Case-insensitive like Windows itself.
pub fn matches_glob(path: &Path, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    let pat = match glob::Pattern::new(&pattern.to_lowercase()) {
        Ok(p) => p,
        Err(_) => return false,
    };
    if pattern.contains('/') || pattern.contains('\\') {
        pat.matches_path(path) || pat.matches(&path.to_string_lossy().to_lowercase())
    } else {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        pat.matches(&name)
    }
}

/// Normalize a path for comparison: forward slashes → backslashes, trimmed
/// trailing separators, lowercased. Windows paths are case-insensitive, so
/// the exclusion check has to be too or "c:\foo" won't protect "C:\Foo".
fn norm_path(p: &str) -> String {
    let mut s = p.replace('/', "\\").to_lowercase();
    while s.ends_with('\\') && s.len() > 3 {
        s.pop();
    }
    s
}

pub fn is_excluded(path: &Path, exclude_dirs: &[String]) -> bool {
    let norm = norm_path(&path.to_string_lossy());
    for ex in exclude_dirs {
        let ex = ex.trim();
        if ex.is_empty() {
            continue;
        }
        let ex = norm_path(ex);
        // Exact match, or a proper child boundary — never a sibling like
        // "C:\KeepOther" matching "C:\Keep".
        if norm == ex
            || (norm.len() > ex.len()
                && norm.starts_with(&ex)
                && norm.as_bytes()[ex.len()] == b'\\')
        {
            return true;
        }
    }
    false
}

// -----------------------------------------------------------------------------
// presentation
// -----------------------------------------------------------------------------

pub fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes < 1024 * 1024 * 1024 * 1024 {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!(
            "{:.2} TB",
            bytes as f64 / (1024.0 * 1024.0 * 1024.0 * 1024.0)
        )
    }
}

// -----------------------------------------------------------------------------
// secure delete
// -----------------------------------------------------------------------------

/// Simple xorshift RNG used only for the 3-pass shredder.
pub struct SimpleRng(pub u64);

impl SimpleRng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

pub fn shred_file(path: &Path) -> Result<(), String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let size = meta.len();
    if size == 0 {
        return fs::remove_file(path).map_err(|e| e.to_string());
    }

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xDEADBEEF);
    let mut rng = SimpleRng::new(seed);

    let mut file = File::create(path).map_err(|e| e.to_string())?;

    let chunk_size: usize = 64 * 1024;
    let mut buf = vec![0u8; chunk_size];

    for _pass in 0..3 {
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut remaining = size;
        while remaining > 0 {
            let n = remaining.min(chunk_size as u64) as usize;
            // Fill 8 bytes at a time — a byte-per-RNG-call shred of a big
            // file is needlessly slow.
            let mut i = 0;
            while i < n {
                let v = rng.next_u64().to_le_bytes();
                let take = (n - i).min(8);
                buf[i..i + take].copy_from_slice(&v[..take]);
                i += take;
            }
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            remaining -= n as u64;
        }
        file.sync_all().map_err(|e| e.to_string())?;
    }

    drop(file);
    fs::remove_file(path).map_err(|e| e.to_string())
}

// -----------------------------------------------------------------------------
// duplicate hashing
// -----------------------------------------------------------------------------

pub fn quick_hash_of_file(path: &Path, size: u64) -> Option<String> {
    use std::io::Read;
    let file = File::open(path).ok()?;
    let mut head = Vec::with_capacity(8192);
    file.take(8192).read_to_end(&mut head).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&head);
    hasher.update(&size.to_le_bytes());
    Some(format!("{:x}", hasher.finalize()))
}

pub fn full_hash_of_file(path: &Path) -> Option<String> {
    use std::io::Read;
    let mut file = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

// -----------------------------------------------------------------------------
// report export
// -----------------------------------------------------------------------------

pub fn export_report(files: &[MatchedFile], label: &str) -> Result<PathBuf, String> {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
    let filename = format!("cleaner_report_{}_{}.txt", label, timestamp);
    let mut content = String::new();
    content.push_str("Cleaner Report\n");
    content.push_str("==============\n");
    content.push_str(&format!("Generated: {}\n", Local::now().format("%Y-%m-%d %H:%M:%S")));
    content.push_str(&format!("Type: {}\n", label));
    content.push_str(&format!("Total files: {}\n", files.len()));
    let total_size: u64 = files.iter().map(|f| f.size).sum();
    content.push_str(&format!("Total size: {}\n\n", human_size(total_size)));
    content.push_str("Files:\n");
    for f in files {
        content.push_str(&format!("  {} - {}\n", human_size(f.size), f.path.display()));
    }
    let path = crate::settings::reports_dir().join(&filename);
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Parse strings produced by `human_size` back into bytes.
/// Only supports the exact formats emitted by this crate.
pub fn parse_human_size(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.ends_with(" B") {
        s.strip_suffix(" B").and_then(|n| n.parse::<u64>().ok())
    } else if s.ends_with(" KB") {
        s.strip_suffix(" KB")
            .and_then(|n| n.parse::<f64>().ok())
            .map(|v| (v * 1024.0) as u64)
    } else if s.ends_with(" MB") {
        s.strip_suffix(" MB")
            .and_then(|n| n.parse::<f64>().ok())
            .map(|v| (v * 1024.0 * 1024.0) as u64)
    } else if s.ends_with(" GB") {
        s.strip_suffix(" GB")
            .and_then(|n| n.parse::<f64>().ok())
            .map(|v| (v * 1024.0 * 1024.0 * 1024.0) as u64)
    } else if s.ends_with(" TB") {
        s.strip_suffix(" TB")
            .and_then(|n| n.parse::<f64>().ok())
            .map(|v| (v * 1024.0 * 1024.0 * 1024.0 * 1024.0) as u64)
    } else {
        None
    }
}

// -----------------------------------------------------------------------------
// large file scan
// -----------------------------------------------------------------------------

pub fn scan_large_files(
    dir_path: &Path,
    threshold_mb: u64,
    exclude: &[String],
    cancel_flag: &AtomicBool,
) -> Option<Vec<LargeFile>> {
    let threshold = threshold_mb.saturating_mul(1024 * 1024);
    let mut large_files = Vec::new();

    for entry in WalkDir::new(dir_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path(), exclude))
    {
        if cancel_flag.load(Ordering::Relaxed) {
            return None;
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
    Some(large_files)
}

// -----------------------------------------------------------------------------
// empty folder scan (cascade-aware, bottom-up)
// -----------------------------------------------------------------------------

pub fn scan_empty_folders(
    dir_path: &Path,
    exclude: &[String],
    cancel_flag: &AtomicBool,
) -> Option<Vec<PathBuf>> {
    let mut all_dirs: Vec<PathBuf> = Vec::new();

    for entry in WalkDir::new(dir_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path(), exclude))
        .filter_map(|e| e.ok())
    {
        if cancel_flag.load(Ordering::Relaxed) {
            return None;
        }
        if entry.file_type().is_dir() {
            all_dirs.push(entry.path().to_path_buf());
        }
    }

    // deepest first so children are processed before parents
    all_dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));

    let mut empty_set: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    for d in &all_dirs {
        if d == dir_path {
            continue;
        }
        if let Ok(rd) = fs::read_dir(d) {
            let mut has_content = false;
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    if !empty_set.contains(&p) {
                        has_content = true;
                        break;
                    }
                } else {
                    has_content = true;
                    break;
                }
            }
            if !has_content {
                empty_set.insert(d.clone());
            }
        }
    }

    let mut empty: Vec<PathBuf> = empty_set.into_iter().collect();
    empty.sort();
    Some(empty)
}

// -----------------------------------------------------------------------------
// Windows Task Scheduler — register/unregister the scheduled scan so it can
// run even while the app is closed. Everything goes through `schtasks.exe`,
// no extra dependencies.
// -----------------------------------------------------------------------------

/// Name used for the scheduled task.
pub const TASK_NAME: &str = "CleanerScheduledScan";

/// Is our scheduled task currently registered?
pub fn task_registered() -> bool {
    std::process::Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Build the argument string the task runs with, e.g. `clean-system --yes`.
/// Honors the same schedule settings the in-app scheduler uses.
pub fn task_command_args(
    target_custom: bool,
    auto_clean: bool,
    custom_dir: &str,
) -> String {
    let mut cmd = match (target_custom, auto_clean) {
        (false, false) => "scan-system".to_string(),
        (false, true) => "clean-system --yes".to_string(),
        (true, false) => format!("scan-custom \"{}\"", custom_dir),
        (true, true) => format!("clean-custom \"{}\" --yes", custom_dir),
    };
    // Scheduled runs must never block on a prompt or touch protected paths.
    if auto_clean && !cmd.contains("--yes") {
        cmd.push_str(" --yes");
    }
    cmd
}

/// Register (or overwrite) the scheduled task. `hours` matches the in-app
/// interval: 1/6/12/24 map to HOURLY, 168 to WEEKLY.
/// Returns Err with the schtasks output on failure.
pub fn register_task(hours: u32, args: &str) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("can't locate exe: {}", e))?;
    let tr = format!("\"{}\" {}", exe.display(), args);

    let mut cmd = std::process::Command::new("schtasks");
    cmd.args(["/Create", "/F", "/TN", TASK_NAME, "/TR", &tr]);
    if hours >= 168 {
        cmd.args(["/SC", "WEEKLY"]);
    } else {
        cmd.args(["/SC", "HOURLY", "/MO", &hours.max(1).to_string()]);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Remove the scheduled task. Ok even if it wasn't there.
pub fn unregister_task() -> Result<(), String> {
    let out = std::process::Command::new("schtasks")
        .args(["/Delete", "/F", "/TN", TASK_NAME])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    // "Task doesn't exist" is not a real failure for unregister purposes.
    let detail = String::from_utf8_lossy(&out.stderr);
    if !task_registered() {
        return Ok(());
    }
    Err(detail.trim().to_string())
}

// -----------------------------------------------------------------------------
// Win32 window control for the tray feature.
//
// egui's viewport command queue is only drained on repaint events, and a
// hidden window never gets any (egui issues #3655 / #5229) — so hiding to
// the tray and coming back has to go through user32 directly instead of
// ViewportCommand::Visible. The HWND is captured once at startup.
// -----------------------------------------------------------------------------

#[cfg(windows)]
mod win32_window {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::Mutex;

    #[link(name = "user32")]
    extern "system" {
        fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
        fn SetForegroundWindow(hwnd: isize) -> i32;
        fn IsIconic(hwnd: isize) -> i32;
        fn PostMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: isize, index: i32, new_value: isize) -> isize;
        fn SetWindowPos(
            hwnd: isize,
            after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
        fn GetWindowPlacement(hwnd: isize, placement: *mut WindowPlacement) -> i32;
        fn SetWindowPlacement(hwnd: isize, placement: *const WindowPlacement) -> i32;
    }

    #[repr(C)]
    struct WindowPlacement {
        length: u32,
        flags: u32,
        show_cmd: u32,
        min_position: [i32; 2],
        max_position: [i32; 2],
        normal_position: [i32; 4],
    }

    const SW_SHOW: i32 = 5;
    const SW_RESTORE: i32 = 9;
    const WM_CLOSE: u32 = 0x0010;
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
    const WS_EX_APPWINDOW: isize = 0x0004_0000;
    const WS_EX_NOACTIVATE: isize = 0x0800_0000;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_FRAMECHANGED: u32 = 0x0020;

    static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
    static SAVED_PLACEMENT: Mutex<Option<WindowPlacement>> = Mutex::new(None);

    /// Store the main window handle (called once from the eframe creation cb).
    pub fn set_main_hwnd(hwnd: isize) {
        MAIN_HWND.store(hwnd, Ordering::Relaxed);
    }

    pub fn have_main_hwnd() -> bool {
        MAIN_HWND.load(Ordering::Relaxed) != 0
    }

    fn hwnd() -> isize {
        MAIN_HWND.load(Ordering::Relaxed)
    }

    /// "Hide" the main window for the tray feature.
    ///
    /// This deliberately does NOT call ShowWindow(SW_HIDE): a truly hidden
    /// window never receives WM_PAINT, so winit stops delivering redraws and
    /// `update()` — which drives scheduled scans and worker polling — is
    /// starved. Instead the window is parked off-screen and given the
    /// tool-window style (no taskbar or alt-tab entry). It stays WS_VISIBLE
    /// from Windows' point of view, so the egui loop keeps running.
    pub fn hide_main_window() {
        let h = hwnd();
        if h == 0 {
            return;
        }
        unsafe {
            let mut wp: WindowPlacement = std::mem::zeroed();
            wp.length = std::mem::size_of::<WindowPlacement>() as u32;
            if GetWindowPlacement(h, &mut wp) != 0 {
                if let Ok(mut slot) = SAVED_PLACEMENT.lock() {
                    *slot = Some(wp);
                }
            }
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
            SetWindowLongPtrW(
                h,
                GWL_EXSTYLE,
                (ex | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) & !WS_EX_APPWINDOW,
            );
            SetWindowPos(
                h,
                0,
                -32000,
                -32000,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }

    /// Restore and focus the main window (tray "Show" / double-click).
    pub fn show_main_window() {
        let h = hwnd();
        if h == 0 {
            return;
        }
        unsafe {
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
            SetWindowLongPtrW(
                h,
                GWL_EXSTYLE,
                (ex & !WS_EX_TOOLWINDOW & !WS_EX_NOACTIVATE) | WS_EX_APPWINDOW,
            );
            if let Ok(mut slot) = SAVED_PLACEMENT.lock() {
                if let Some(wp) = slot.take() {
                    SetWindowPlacement(h, &wp);
                }
            }
            if IsIconic(h) != 0 {
                ShowWindow(h, SW_RESTORE);
            } else {
                ShowWindow(h, SW_SHOW);
            }
            SetWindowPos(
                h,
                0,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
            SetForegroundWindow(h);
        }
    }

    /// Ask the app to close gracefully (winit delivers it like a normal X).
    pub fn close_main_window() {
        let h = hwnd();
        if h != 0 {
            unsafe {
                PostMessageW(h, WM_CLOSE, 0, 0);
            }
        }
    }
}

#[cfg(windows)]
pub use win32_window::*;

/// Non-Windows stubs — the tray button simply hides via egui there and this
/// code is never exercised on other platforms.
#[cfg(not(windows))]
pub fn set_main_hwnd(_hwnd: isize) {}
#[cfg(not(windows))]
pub fn have_main_hwnd() -> bool {
    false
}
#[cfg(not(windows))]
pub fn hide_main_window() {}
#[cfg(not(windows))]
pub fn show_main_window() {}
#[cfg(not(windows))]
pub fn close_main_window() {}

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

pub fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

pub fn collect_files(dir: &Path, recursive: bool, include_hidden: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if recursive && (include_hidden || !is_hidden(&path)) {
                    files.extend(collect_files(&path, recursive, include_hidden));
                }
            } else if include_hidden || !is_hidden(&path) {
                files.push(path);
            }
        }
    }
    files
}

pub fn matches_glob(path: &Path, pattern: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    if let Ok(pat) = glob::Pattern::new(pattern) {
        pat.matches_path(path)
    } else {
        false
    }
}

pub fn is_excluded(path: &Path, exclude_dirs: &[String]) -> bool {
    for ex in exclude_dirs {
        if !ex.trim().is_empty() && path.starts_with(ex.trim()) {
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
            for i in 0..n {
                buf[i] = (rng.next_u64() & 0xFF) as u8;
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
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
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
    fs::write(&filename, content).map_err(|e| e.to_string())?;
    Ok(PathBuf::from(filename))
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
    cancel_flag: &AtomicBool,
) -> Option<Vec<LargeFile>> {
    let threshold = threshold_mb.saturating_mul(1024 * 1024);
    let mut large_files = Vec::new();

    for entry in WalkDir::new(dir_path).follow_links(false) {
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

pub fn scan_empty_folders(dir_path: &Path, cancel_flag: &AtomicBool) -> Option<Vec<PathBuf>> {
    let mut all_dirs: Vec<PathBuf> = Vec::new();

    for entry in WalkDir::new(dir_path)
        .follow_links(false)
        .into_iter()
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

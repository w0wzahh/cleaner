//! Background scanning/cleaning workers.
//!
//! Each worker runs on its own thread and talks back to the UI through an
//! `mpsc::Sender<WorkerMessage>`.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

use walkdir::WalkDir;

use crate::helpers;
use crate::models::*;

// -----------------------------------------------------------------------------
// message protocol
// -----------------------------------------------------------------------------

#[derive(Debug)]
pub enum WorkerMessage {
    Log(String),
    Progress(f32),
    CustomMatched(Vec<MatchedFile>),
    Duplicates(Vec<DuplicateGroup>),
    LargeFiles(Vec<LargeFile>),
    SystemMatched(Vec<MatchedFile>),
    EmptyFolders(Vec<PathBuf>),
    FolderSizes(Vec<FolderSizeEntry>),
    /// Structured clean result so the UI doesn't have to parse summary strings.
    /// `paths` are the items that were actually deleted (for pruning results).
    CleanStats { deleted: u64, freed: u64, paths: Vec<PathBuf> },
    Done { summary: String },
    Error(String),
    Cancelled,
}

// -----------------------------------------------------------------------------
// custom scan
// -----------------------------------------------------------------------------

pub fn custom_scan_worker(
    dir_path: String,
    extensions: String,
    older_than_days: u64,
    min_size_bytes: u64,
    max_size_bytes: u64,
    pattern: String,
    exclude_dirs: String,
    protected: Vec<String>,
    recursive: bool,
    include_hidden: bool,
    cancel_flag: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerMessage>,
) {
    let dir = PathBuf::from(&dir_path);
    if !dir.exists() {
        let _ = tx.send(WorkerMessage::Error(format!(
            "Directory does not exist: {}",
            dir.display()
        )));
        return;
    }
    if !dir.is_dir() {
        let _ = tx.send(WorkerMessage::Error(format!(
            "Not a directory: {}",
            dir.display()
        )));
        return;
    }

    let exts: Vec<String> = extensions
        .split(',')
        .map(|s| s.trim().trim_start_matches('.').to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    let mut exclude: Vec<String> = exclude_dirs
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    // Protected paths are never even matched.
    exclude.extend(protected);

    let _ = tx.send(WorkerMessage::Log("Scanning files...".to_string()));
    let files = match helpers::collect_files(
        &dir,
        recursive,
        include_hidden,
        &exclude,
        &cancel_flag,
    ) {
        Some(f) => f,
        None => {
            let _ = tx.send(WorkerMessage::Cancelled);
            return;
        }
    };
    let total = files.len();
    let _ = tx.send(WorkerMessage::Log(format!("Found {} files", total)));

    let mut matched = Vec::new();
    for (i, file) in files.iter().enumerate() {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = tx.send(WorkerMessage::Cancelled);
            return;
        }
        if i % 10 == 0 || i + 1 == total {
            let _ = tx.send(WorkerMessage::Progress((i + 1) as f32 / total.max(1) as f32));
        }
        if !exts.is_empty() {
            let ext = file
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());
            if !ext.map(|e| exts.contains(&e)).unwrap_or(false) {
                continue;
            }
        }
        if !pattern.is_empty() && !helpers::matches_glob(file, &pattern) {
            continue;
        }
        if older_than_days > 0 {
            if let Some(age) = helpers::file_age_days(file) {
                if age < older_than_days {
                    continue;
                }
            } else {
                continue;
            }
        }
        let size = if let Ok(meta) = fs::metadata(file) {
            meta.len()
        } else {
            0
        };
        if size < min_size_bytes {
            continue;
        }
        if max_size_bytes > 0 && size > max_size_bytes {
            continue;
        }
        matched.push(MatchedFile {
            path: file.clone(),
            size,
        });
    }

    let matched_count = matched.len();
    let total_size: u64 = matched.iter().map(|f| f.size).sum();
    let _ = tx.send(WorkerMessage::Log(format!(
        "Matched {} files, total {}",
        matched_count,
        helpers::human_size(total_size)
    )));
    let _ = tx.send(WorkerMessage::CustomMatched(matched));
    let _ = tx.send(WorkerMessage::Done {
        summary: format!("Scan complete. {} files matched.", matched_count),
    });
}

// -----------------------------------------------------------------------------
// duplicate scan (two-stage)
// -----------------------------------------------------------------------------

pub fn duplicates_worker(
    dir_path: String,
    excludes: Vec<String>,
    cancel_flag: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerMessage>,
) {
    let dir = PathBuf::from(&dir_path);
    if !dir.exists() || !dir.is_dir() {
        let _ = tx.send(WorkerMessage::Error(format!(
            "Invalid directory: {}",
            dir.display()
        )));
        return;
    }

    let _ = tx.send(WorkerMessage::Log("Scanning for duplicates (optimized)...".to_string()));

    let mut size_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut total_files = 0;
    let mut skipped_excluded = 0u64;

    for entry in WalkDir::new(&dir).follow_links(false) {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = tx.send(WorkerMessage::Cancelled);
            return;
        }
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() {
                if helpers::is_excluded(path, &excludes) {
                    skipped_excluded += 1;
                    continue;
                }
                if let Ok(meta) = fs::metadata(path) {
                    // Zero-byte files all hash identically and aren't worth reporting.
                    if meta.len() == 0 {
                        continue;
                    }
                    size_map.entry(meta.len()).or_default().push(path.to_path_buf());
                    total_files += 1;
                }
            }
        }
    }

    let _ = tx.send(WorkerMessage::Log(format!(
        "Found {} files{}, grouping by size...",
        total_files,
        if skipped_excluded > 0 {
            format!(" ({} skipped — excluded)", skipped_excluded)
        } else {
            String::new()
        }
    )));
    let _ = tx.send(WorkerMessage::Progress(0.1));

    let mut groups = Vec::new();
    let mut processed_groups = 0;
    let total_groups = size_map.len();

    for (_size, files) in size_map {
        if files.len() < 2 {
            processed_groups += 1;
            continue;
        }

        let mut quick_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for file in &files {
            if cancel_flag.load(Ordering::Relaxed) {
                let _ = tx.send(WorkerMessage::Cancelled);
                return;
            }
            if let Some(quick_hash) = helpers::quick_hash_of_file(file, _size) {
                quick_map.entry(quick_hash).or_default().push(file.clone());
            }
        }

        for (_key, quick_files) in quick_map {
            if quick_files.len() < 2 {
                continue;
            }
            let mut full_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
            for file in &quick_files {
                if cancel_flag.load(Ordering::Relaxed) {
                    let _ = tx.send(WorkerMessage::Cancelled);
                    return;
                }
                if let Some(hash_str) = helpers::full_hash_of_file(file) {
                    full_map.entry(hash_str).or_default().push(file.clone());
                }
            }
            for (hash, mut full_files) in full_map {
                if full_files.len() > 1 {
                    // Deterministic order so the "kept" file is stable run to run.
                    full_files.sort();
                    groups.push(DuplicateGroup {
                        hash,
                        files: full_files,
                        size: _size,
                    });
                }
            }
        }

        processed_groups += 1;
        let progress = 0.1 + 0.85 * (processed_groups as f32 / total_groups.max(1) as f32);
        let _ = tx.send(WorkerMessage::Progress(progress));
        if processed_groups % 10 == 0 {
            let _ = tx.send(WorkerMessage::Log(format!(
                "Processed {}/{} groups...",
                processed_groups, total_groups
            )));
        }
    }

    let groups_len = groups.len();
    let total_wasted: u64 =
        groups.iter().map(|g| g.size * (g.files.len() as u64 - 1)).sum();
    let _ = tx.send(WorkerMessage::Progress(0.97));
    let _ = tx.send(WorkerMessage::Log(format!(
        "Found {} duplicate groups, wasting {}",
        groups_len,
        helpers::human_size(total_wasted)
    )));
    let _ = tx.send(WorkerMessage::Duplicates(groups));
    let _ = tx.send(WorkerMessage::Progress(1.0));
    let _ = tx.send(WorkerMessage::Done {
        summary: format!("Duplicate scan complete. Found {} groups.", groups_len),
    });
}

// -----------------------------------------------------------------------------
// large files scan
// -----------------------------------------------------------------------------

pub fn large_files_worker(
    dir_path: String,
    threshold_mb: u64,
    protected: Vec<String>,
    cancel_flag: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerMessage>,
) {
    let dir = PathBuf::from(&dir_path);
    if !dir.exists() || !dir.is_dir() {
        let _ = tx.send(WorkerMessage::Error(format!(
            "Invalid directory: {}",
            dir.display()
        )));
        return;
    }

    let _ = tx.send(WorkerMessage::Log(format!(
        "Scanning for files larger than {} MB...",
        threshold_mb
    )));

    let large_files =
        match helpers::scan_large_files(&dir, threshold_mb, &protected, &cancel_flag) {
            Some(files) => files,
            None => {
                let _ = tx.send(WorkerMessage::Cancelled);
                return;
            }
        };
    let total_size: u64 = large_files.iter().map(|f| f.size).sum();
    let _ = tx.send(WorkerMessage::Log(format!(
        "Found {} large files, total {}",
        large_files.len(),
        helpers::human_size(total_size)
    )));
    let _ = tx.send(WorkerMessage::LargeFiles(large_files));
    let _ = tx.send(WorkerMessage::Done {
        summary: "Large file scan complete.".to_string(),
    });
}

// -----------------------------------------------------------------------------
// system scan
// -----------------------------------------------------------------------------

pub fn system_scan_worker(
    targets: Vec<SystemCleanTarget>,
    protected: Vec<String>,
    cancel_flag: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerMessage>,
) {
    let mut matched = Vec::new();
    for target in targets {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = tx.send(WorkerMessage::Cancelled);
            return;
        }
        if !target.enabled || helpers::is_excluded(&target.path, &protected) {
            continue;
        }
        let _ = tx.send(WorkerMessage::Log(format!("Scanning {} ...", target.name)));
        // Recurse: temp/cache targets keep most of their junk in subfolders.
        for entry in WalkDir::new(&target.path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !helpers::is_excluded(e.path(), &protected))
        {
            if cancel_flag.load(Ordering::Relaxed) {
                let _ = tx.send(WorkerMessage::Cancelled);
                return;
            }
            if let Ok(entry) = entry {
                let path = entry.path();
                if let Ok(meta) = fs::metadata(path) {
                    if meta.is_file() {
                        matched.push(MatchedFile {
                            path: path.to_path_buf(),
                            size: meta.len(),
                        });
                    }
                }
            }
        }
    }
    let total_size: u64 = matched.iter().map(|f| f.size).sum();
    let _ = tx.send(WorkerMessage::Log(format!(
        "System scan found {} files, total {}",
        matched.len(),
        helpers::human_size(total_size)
    )));
    let _ = tx.send(WorkerMessage::SystemMatched(matched));
    let _ = tx.send(WorkerMessage::Done {
        summary: "System scan complete.".to_string(),
    });
}

// -----------------------------------------------------------------------------
// empty folders scan
// -----------------------------------------------------------------------------

pub fn empty_folders_worker(
    dir_path: String,
    protected: Vec<String>,
    cancel_flag: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerMessage>,
) {
    let dir = PathBuf::from(&dir_path);
    if !dir.exists() || !dir.is_dir() {
        let _ = tx.send(WorkerMessage::Error(format!(
            "Invalid directory: {}",
            dir.display()
        )));
        return;
    }

    let _ = tx.send(WorkerMessage::Log("Scanning for empty folders...".to_string()));

    let empty = match helpers::scan_empty_folders(&dir, &protected, &cancel_flag) {
        Some(folders) => folders,
        None => {
            let _ = tx.send(WorkerMessage::Cancelled);
            return;
        }
    };
    let _ = tx.send(WorkerMessage::Log(format!(
        "Found {} empty folders",
        empty.len()
    )));
    let _ = tx.send(WorkerMessage::EmptyFolders(empty));
    let _ = tx.send(WorkerMessage::Done {
        summary: "Empty folder scan complete.".to_string(),
    });
}

// -----------------------------------------------------------------------------
// folder size breakdown
// -----------------------------------------------------------------------------

/// Aggregates sizes by top-level entry under `dir_path` — answers
/// "which subfolder is eating my disk?".
pub fn folder_sizes_worker(
    dir_path: String,
    protected: Vec<String>,
    cancel_flag: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerMessage>,
) {
    let dir = PathBuf::from(&dir_path);
    if !dir.exists() || !dir.is_dir() {
        let _ = tx.send(WorkerMessage::Error(format!(
            "Invalid directory: {}",
            dir.display()
        )));
        return;
    }

    let _ = tx.send(WorkerMessage::Log("Analyzing folder sizes...".to_string()));

    let mut map: HashMap<String, u64> = HashMap::new();
    let mut total = 0u64;
    let mut seen = 0usize;

    for entry in WalkDir::new(&dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !helpers::is_excluded(e.path(), &protected))
    {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = tx.send(WorkerMessage::Cancelled);
            return;
        }
        if let Ok(entry) = entry {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Ok(meta) = fs::metadata(path) {
                let size = meta.len();
                let key = path
                    .strip_prefix(&dir)
                    .ok()
                    .and_then(|rel| {
                        let mut it = rel.components();
                        let first = it.next()?;
                        Some(if it.next().is_none() {
                            "(files in root)".to_string()
                        } else {
                            first.as_os_str().to_string_lossy().into_owned()
                        })
                    })
                    .unwrap_or_else(|| "(other)".to_string());
                *map.entry(key).or_default() += size;
                total += size;
                seen += 1;
                if seen % 5000 == 0 {
                    let _ = tx.send(WorkerMessage::Log(format!(
                        "Counted {} files...",
                        seen
                    )));
                }
            }
        }
    }

    let mut entries: Vec<FolderSizeEntry> = map
        .into_iter()
        .map(|(name, size)| FolderSizeEntry { name, size })
        .collect();
    entries.sort_by(|a, b| b.size.cmp(&a.size));
    entries.truncate(40);

    let _ = tx.send(WorkerMessage::Log(format!(
        "Breakdown complete: {} files, {}",
        seen,
        helpers::human_size(total)
    )));
    let _ = tx.send(WorkerMessage::FolderSizes(entries));
    let _ = tx.send(WorkerMessage::Done {
        summary: format!(
            "Folder size analysis complete. {} files, {} total.",
            seen,
            helpers::human_size(total)
        ),
    });
}

// -----------------------------------------------------------------------------
// cleaning
// -----------------------------------------------------------------------------

pub fn clean_files(
    files: Vec<MatchedFile>,
    use_trash: bool,
    dry_run: bool,
    secure_delete: bool,
    protected: Vec<String>,
    cancel_flag: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerMessage>,
) {
    let total = files.len();
    let mut deleted = 0;
    let mut errors = 0;
    let mut skipped = 0;
    let mut freed = 0u64;
    let mut deleted_paths = Vec::new();

    for (i, file) in files.iter().enumerate() {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = tx.send(WorkerMessage::Cancelled);
            return;
        }
        if i % 10 == 0 || i + 1 == total {
            let _ = tx.send(WorkerMessage::Progress((i + 1) as f32 / total.max(1) as f32));
        }

        if helpers::is_excluded(&file.path, &protected) {
            skipped += 1;
            let _ = tx.send(WorkerMessage::Log(format!(
                "Skipped (protected): {}",
                file.path.display()
            )));
            continue;
        }

        if dry_run {
            let _ = tx.send(WorkerMessage::Log(format!(
                "[DRY] Would delete: {}",
                file.path.display()
            )));
            freed += file.size;
            deleted += 1;
            deleted_paths.push(file.path.clone());
            continue;
        }

        let result: Result<(), String> = if secure_delete {
            helpers::shred_file(&file.path)
        } else if use_trash {
            trash::delete(&file.path).map_err(|e| e.to_string())
        } else {
            fs::remove_file(&file.path).map_err(|e| e.to_string())
        };

        match result {
            Ok(_) => {
                deleted += 1;
                freed += file.size;
                deleted_paths.push(file.path.clone());
                let _ = tx.send(WorkerMessage::Log(format!(
                    "Deleted: {}",
                    file.path.display()
                )));
            }
            Err(e) => {
                if !file.path.exists() {
                    // Already gone — temp files vanish on their own all the
                    // time; the goal state is reached, so don't scare the
                    // user with an "error".
                    deleted += 1;
                    freed += file.size;
                    deleted_paths.push(file.path.clone());
                } else {
                    errors += 1;
                    let _ = tx.send(WorkerMessage::Log(format!(
                        "Error deleting {}: {}",
                        file.path.display(),
                        e
                    )));
                }
            }
        }
    }

    let skipped_note = if skipped > 0 {
        format!(" {} protected skipped.", skipped)
    } else {
        String::new()
    };
    let summary = if dry_run {
        format!(
            "Dry run complete. Would delete {} files, freeing {}.{}",
            deleted,
            helpers::human_size(freed),
            skipped_note
        )
    } else {
        let _ = tx.send(WorkerMessage::CleanStats {
            deleted,
            freed,
            paths: deleted_paths,
        });
        format!(
            "Cleaning complete. Deleted {} files, freed {}, {} errors.{}",
            deleted,
            helpers::human_size(freed),
            errors,
            skipped_note
        )
    };
    let _ = tx.send(WorkerMessage::Done { summary });
}

pub fn clean_folders(
    folders: Vec<PathBuf>,
    dry_run: bool,
    use_trash: bool,
    protected: Vec<String>,
    cancel_flag: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerMessage>,
) {
    let total = folders.len();
    let mut deleted = 0;
    let mut errors = 0;
    let mut skipped = 0;
    let mut deleted_paths = Vec::new();

    let mut sorted = folders;
    sorted.sort_by_key(|p| std::cmp::Reverse(p.components().count()));

    for (i, folder) in sorted.iter().enumerate() {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = tx.send(WorkerMessage::Cancelled);
            return;
        }
        if i % 10 == 0 || i + 1 == total {
            let _ = tx.send(WorkerMessage::Progress((i + 1) as f32 / total.max(1) as f32));
        }

        if helpers::is_excluded(folder, &protected) {
            skipped += 1;
            let _ = tx.send(WorkerMessage::Log(format!(
                "Skipped (protected): {}",
                folder.display()
            )));
            continue;
        }

        if dry_run {
            let _ = tx.send(WorkerMessage::Log(format!(
                "[DRY] Would remove: {}",
                folder.display()
            )));
            deleted += 1;
            deleted_paths.push(folder.clone());
            continue;
        }

        // Re-check emptiness: a file may have landed here between the scan
        // and the clean — never trash a folder that now has contents.
        let still_empty = fs::read_dir(folder)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false);
        if !still_empty {
            let _ = tx.send(WorkerMessage::Log(format!(
                "Skipped (no longer empty): {}",
                folder.display()
            )));
            skipped += 1;
            continue;
        }

        let result = if use_trash {
            trash::delete(folder).map_err(|e| e.to_string())
        } else {
            fs::remove_dir(folder).map_err(|e| e.to_string())
        };

        match result {
            Ok(_) => {
                deleted += 1;
                deleted_paths.push(folder.clone());
                let _ = tx.send(WorkerMessage::Log(format!(
                    "Removed: {}",
                    folder.display()
                )));
            }
            Err(e) => {
                if !folder.exists() {
                    deleted += 1;
                    deleted_paths.push(folder.clone());
                } else {
                    errors += 1;
                    let _ = tx.send(WorkerMessage::Log(format!(
                        "Error removing {}: {}",
                        folder.display(),
                        e
                    )));
                }
            }
        }
    }

    let skipped_note = if skipped > 0 {
        format!(" {} protected skipped.", skipped)
    } else {
        String::new()
    };
    let summary = if dry_run {
        format!(
            "Dry run complete. Would remove {} empty folders.{}",
            deleted, skipped_note
        )
    } else {
        let _ = tx.send(WorkerMessage::CleanStats {
            deleted,
            freed: 0,
            paths: deleted_paths,
        });
        format!(
            "Removed {} empty folders, {} errors.{}",
            deleted, errors, skipped_note
        )
    };
    let _ = tx.send(WorkerMessage::Done { summary });
}

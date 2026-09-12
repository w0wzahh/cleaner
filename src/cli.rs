//! Headless command-line mode.
//!
//! `cleaner` with no arguments launches the GUI. Any arguments run a one-shot
//! command instead — useful for scripting and Windows Task Scheduler.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;

use crate::helpers;
use crate::models::*;
use crate::settings::Settings;
use crate::workers::{self, WorkerMessage};

const USAGE: &str = r#"Cleaner — command-line mode

USAGE:
  cleaner                     Launch the GUI
  cleaner <command> [flags]   Run headless

SCAN COMMANDS (preview only):
  scan-system                    Scan the enabled system-clean targets
  scan-custom <dir>              Scan a folder with filters
  scan-duplicates <dir>          Find duplicate files
  scan-large <dir>               Find large files
  scan-empty <dir>               Find empty folders
  scan-folders <dir>             Folder-size breakdown

CLEAN COMMANDS (delete for real):
  clean-system [--yes]           Clean enabled system targets
  clean-custom <dir> [filters] [--yes]
  clean-duplicates <dir> [--yes] Keeps the first file in each group
  clean-large <dir> [--min-mb N] [--yes]
  clean-empty <dir> [--yes]

FILTERS (custom / large scans):
  --ext a,b,c        Extensions (default: tmp,log)
  --pattern <glob>   Glob pattern, e.g. "*.tmp"
  --older-than <d>   Only files older than N days
  --min-size <b>     Minimum file size in bytes
  --max-size <b>     Maximum file size in bytes (0 = no limit)
  --min-mb <n>       Large-file threshold in MB (default: 100)
  --recursive        Include subdirectories
  --hidden           Include hidden files

OTHER FLAGS:
  --yes              Skip the confirmation prompt (for automation)
  --permanent        Delete permanently instead of using the recycle bin
  --secure           3-pass overwrite before deletion
"#;

/// Spawn a worker on a thread and drain its messages until it finishes.
/// Log lines are echoed to stdout as they arrive.
fn run_and_collect(
    spawn: impl FnOnce(mpsc::Sender<WorkerMessage>, Arc<AtomicBool>) + Send + 'static,
) -> Vec<WorkerMessage> {
    let (tx, rx) = mpsc::channel::<WorkerMessage>();
    let cancel = Arc::new(AtomicBool::new(false));
    std::thread::spawn(move || spawn(tx, cancel));
    let mut msgs = Vec::new();
    while let Ok(m) = rx.recv() {
        if let WorkerMessage::Log(s) = &m {
            println!("{}", s);
        }
        let finished = matches!(
            m,
            WorkerMessage::Done { .. } | WorkerMessage::Error(_) | WorkerMessage::Cancelled
        );
        msgs.push(m);
        if finished {
            break;
        }
    }
    msgs
}

fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn flag_u64(args: &[String], name: &str, default: u64) -> u64 {
    flag_value(args, name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// First positional argument (the one right after the command name).
fn dir_arg(args: &[String]) -> Option<String> {
    args.get(1).filter(|a| !a.starts_with("--")).cloned()
}

fn print_done(msgs: &[WorkerMessage]) -> i32 {
    for m in msgs {
        match m {
            WorkerMessage::Done { summary } => {
                println!("{}", summary);
                return 0;
            }
            WorkerMessage::Error(e) => {
                eprintln!("Error: {}", e);
                return 1;
            }
            _ => {}
        }
    }
    0
}

fn confirm(prompt: &str) -> bool {
    eprint!("{} [y/N]: ", prompt);
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}

/// Run a file clean after a scan produced `matched`.
fn clean_flow(
    matched: Vec<MatchedFile>,
    yes: bool,
    settings: &Settings,
    permanent: bool,
    secure: bool,
) -> i32 {
    if matched.is_empty() {
        println!("Nothing to clean.");
        return 0;
    }
    let total: u64 = matched.iter().map(|f| f.size).sum();
    println!(
        "Matched {} files, {}.",
        matched.len(),
        helpers::human_size(total)
    );
    if !yes && !confirm("Delete these files?") {
        println!("Aborted.");
        return 0;
    }
    let use_trash = settings.use_trash && !permanent;
    let secure = secure || settings.secure_delete;
    let protected = settings.protected_list();
    let msgs = run_and_collect(move |tx, cancel| {
        workers::clean_files(matched, use_trash, false, secure, protected, cancel, tx)
    });
    print_done(&msgs)
}

/// Same idea for folder removal.
fn clean_folders_flow(
    folders: Vec<PathBuf>,
    yes: bool,
    settings: &Settings,
    permanent: bool,
) -> i32 {
    if folders.is_empty() {
        println!("Nothing to remove.");
        return 0;
    }
    println!("Found {} empty folders.", folders.len());
    if !yes && !confirm("Remove these folders?") {
        println!("Aborted.");
        return 0;
    }
    let use_trash = settings.use_trash && !permanent;
    let protected = settings.protected_list();
    let msgs = run_and_collect(move |tx, cancel| {
        workers::clean_folders(folders, false, use_trash, protected, cancel, tx)
    });
    print_done(&msgs)
}

fn print_files(files: &[MatchedFile]) {
    for f in files {
        println!("{:>12}  {}", helpers::human_size(f.size), f.path.display());
    }
}

pub fn run(args: &[String]) -> i32 {
    let settings = Settings::load();
    let yes = has_flag(args, "--yes");
    let permanent = has_flag(args, "--permanent");
    let secure = has_flag(args, "--secure");

    match args.first().map(|s| s.as_str()) {
        Some("help") | Some("--help") | Some("-h") => {
            print!("{}", USAGE);
            0
        }

        Some("scan-system") | Some("clean-system") => {
            let clean = args[0].starts_with("clean");
            let targets = SystemCleanerState::default().targets;
            let msgs = run_and_collect(move |tx, cancel| {
                workers::system_scan_worker(targets, cancel, tx)
            });
            let files: Vec<MatchedFile> = msgs
                .iter()
                .find_map(|m| match m {
                    WorkerMessage::SystemMatched(f) => Some(f.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            if clean {
                clean_flow(files, yes, &settings, permanent, secure)
            } else {
                print_files(&files);
                print_done(&msgs)
            }
        }

        Some("scan-custom") | Some("clean-custom") => {
            let clean = args[0].starts_with("clean");
            let dir = match dir_arg(args) {
                Some(d) => d,
                None => {
                    eprintln!("scan-custom needs a directory. Try `cleaner help`.");
                    return 2;
                }
            };
            let exts = flag_value(args, "--ext").unwrap_or_else(|| "tmp,log".into());
            let pattern = flag_value(args, "--pattern").unwrap_or_default();
            let older = flag_u64(args, "--older-than", 0);
            let min = flag_u64(args, "--min-size", 0);
            let max = flag_u64(args, "--max-size", 0);
            let recursive = has_flag(args, "--recursive") || settings.recursive;
            let hidden = has_flag(args, "--hidden") || settings.include_hidden;
            let protected = settings.protected_list();
            let msgs = run_and_collect(move |tx, cancel| {
                workers::custom_scan_worker(
                    dir,
                    exts,
                    older,
                    min,
                    max,
                    pattern,
                    String::new(),
                    protected,
                    recursive,
                    hidden,
                    cancel,
                    tx,
                )
            });
            let files: Vec<MatchedFile> = msgs
                .iter()
                .find_map(|m| match m {
                    WorkerMessage::CustomMatched(f) => Some(f.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            if clean {
                clean_flow(files, yes, &settings, permanent, secure)
            } else {
                print_files(&files);
                print_done(&msgs)
            }
        }

        Some("scan-duplicates") | Some("clean-duplicates") => {
            let clean = args[0].starts_with("clean");
            let dir = match dir_arg(args) {
                Some(d) => d,
                None => {
                    eprintln!("scan-duplicates needs a directory. Try `cleaner help`.");
                    return 2;
                }
            };
            let msgs = run_and_collect(move |tx, cancel| {
                workers::duplicates_worker(dir, cancel, tx)
            });
            let groups: Vec<DuplicateGroup> = msgs
                .iter()
                .find_map(|m| match m {
                    WorkerMessage::Duplicates(g) => Some(g.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            if clean {
                // Keep the first file in each group, delete the rest.
                let matched: Vec<MatchedFile> = groups
                    .iter()
                    .flat_map(|g| {
                        g.files
                            .iter()
                            .skip(1)
                            .map(|p| MatchedFile {
                                path: p.clone(),
                                size: g.size,
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect();
                clean_flow(matched, yes, &settings, permanent, secure)
            } else {
                for g in &groups {
                    println!(
                        "-- {} files, {} each:",
                        g.files.len(),
                        helpers::human_size(g.size)
                    );
                    for f in &g.files {
                        println!("   {}", f.display());
                    }
                }
                print_done(&msgs)
            }
        }

        Some("scan-large") | Some("clean-large") => {
            let clean = args[0].starts_with("clean");
            let dir = match dir_arg(args) {
                Some(d) => d,
                None => {
                    eprintln!("scan-large needs a directory. Try `cleaner help`.");
                    return 2;
                }
            };
            let threshold = flag_u64(args, "--min-mb", 100);
            let msgs = run_and_collect(move |tx, cancel| {
                workers::large_files_worker(dir, threshold, cancel, tx)
            });
            let files: Vec<MatchedFile> = msgs
                .iter()
                .find_map(|m| match m {
                    WorkerMessage::LargeFiles(f) => Some(
                        f.iter()
                            .map(|lf| MatchedFile {
                                path: lf.path.clone(),
                                size: lf.size,
                            })
                            .collect(),
                    ),
                    _ => None,
                })
                .unwrap_or_default();
            if clean {
                clean_flow(files, yes, &settings, permanent, secure)
            } else {
                print_files(&files);
                print_done(&msgs)
            }
        }

        Some("scan-empty") | Some("clean-empty") => {
            let clean = args[0].starts_with("clean");
            let dir = match dir_arg(args) {
                Some(d) => d,
                None => {
                    eprintln!("scan-empty needs a directory. Try `cleaner help`.");
                    return 2;
                }
            };
            let msgs = run_and_collect(move |tx, cancel| {
                workers::empty_folders_worker(dir, cancel, tx)
            });
            let folders: Vec<PathBuf> = msgs
                .iter()
                .find_map(|m| match m {
                    WorkerMessage::EmptyFolders(f) => Some(f.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            if clean {
                clean_folders_flow(folders, yes, &settings, permanent)
            } else {
                for f in &folders {
                    println!("{}", f.display());
                }
                print_done(&msgs)
            }
        }

        Some("scan-folders") => {
            let dir = match dir_arg(args) {
                Some(d) => d,
                None => {
                    eprintln!("scan-folders needs a directory. Try `cleaner help`.");
                    return 2;
                }
            };
            let msgs = run_and_collect(move |tx, cancel| {
                workers::folder_sizes_worker(dir, cancel, tx)
            });
            for m in &msgs {
                if let WorkerMessage::FolderSizes(entries) = m {
                    for e in entries {
                        println!("{:>12}  {}", helpers::human_size(e.size), e.name);
                    }
                }
            }
            print_done(&msgs)
        }

        Some(other) => {
            eprintln!("Unknown command: {}", other);
            print!("{}", USAGE);
            2
        }
        None => {
            print!("{}", USAGE);
            2
        }
    }
}

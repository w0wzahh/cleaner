# Cleaner

A fast, safe system cleaner for Windows — written in Rust.

Cleaner digs through a folder (or the usual junk spots on your system) and
shows you exactly what's wasting space — duplicates, huge files, stale temp
data, empty folders — before it deletes anything. Dry-run mode is on by
default and files go to the recycle bin unless you say otherwise, so you can
look before you leap.

## What it does

- **Custom clean** — point it at any folder and filter by extension, glob
  pattern, age, or size. You get a full preview before anything is touched.
- **Duplicate finder** — groups files by size, quick-hashes the first 8 KB,
  and only runs a full SHA-256 when it actually has to. Finds dupes fast
  without hashing your entire drive.
- **Large file finder** — everything above a size you pick, biggest first,
  with one-click reveal in Explorer.
- **System cleaner** — temp folders, app caches, browser caches (Chrome,
  Edge, Firefox), and the Windows thumbnail cache. You pick which targets
  are in scope.
- **Empty folder cleaner** — finds empty directories, including ones that
  only become empty once their children are removed.
- **Folder sizes** — ranks a folder's subfolders by total size so you can
  see exactly where the space went.
- **Storage overview** — per-drive usage bars so you can see what's full.
- **Command line** — `cleaner scan-custom <dir>`, `cleaner clean-system --yes`
  and friends, for scripting or Task Scheduler. `cleaner help` lists them all.

Safety, since that's the part that matters:

- Dry run is on out of the box — nothing is deleted until you turn it off
- Deletion goes to the recycle bin by default; permanent delete is opt-in
- Every clean asks for confirmation first
- Optional 3-pass secure erase for files you really want gone
- A protected-paths list that's never scanned or deleted, period
- A persistent log records every operation

## Install

Grab `cleaner-vX.X.X-windows-x64.zip` from the
[Releases page](https://github.com/w0wzahh/cleaner/releases), unzip it, run
`cleaner.exe`. No installer, no dependencies.

## Using it

1. Pick a tool from the sidebar (Dashboard is a good first stop)
2. Choose a folder and tweak the filters
3. Hit **Scan** — this only previews matches
4. Look over the results, then hit **Clean**

Five themes are in the header bar — Dark, Light, Nord, Dracula, Solarized —
and switching between them animates instead of snapping.

## Settings

Cleaner writes `cleaner_settings.json` next to the exe on first run:

| Field | What it does |
|---|---|
| `theme` | `Dark`, `Light`, `Nord`, `Dracula`, or `Solarized` |
| `dry_run` | Preview matches without deleting anything |
| `use_trash` | Send deleted files to the recycle bin |
| `recursive` | Include subdirectories in custom scans |
| `include_hidden` | Include dotfiles and hidden folders |
| `confirm_clean` | Ask before deleting |
| `secure_delete` | Overwrite files 3 times before removing |
| `default_dir` | Folder the scan tabs start in |
| `github_url` | Where the About → Open button goes |
| `log_file` | Path of the persistent history log |
| `protected_paths` | Folders that are never scanned or deleted |
| `custom_targets` | Your own entries in the System Cleaner list |
| `total_files_cleaned` / `total_space_freed` | Lifetime dashboard counters |

## Building it yourself

You'll need [Rust](https://rustup.rs/). Then:

```bash
cargo build --release
```

The binary lands at `target\release\cleaner.exe`. `cargo run` works for
development.

## Changelog

See [CHANGELOG.md](CHANGELOG.md). Current version is **2.5.0**.

## License

MIT — see [LICENSE](LICENSE). Do whatever you want with it.

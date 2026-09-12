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
- **Scheduled scans** — run a scan every hour, 6/12/24 hours, or weekly while
  the app is open, with an optional auto-clean afterwards. Can also register
  a Windows Task Scheduler entry so scans run even when the app is closed.
- **System tray** — "To tray" hides the window; Cleaner keeps running (and
  keeps its schedule) in the tray. Double-click the icon to bring it back.
- **Sortable results** — order results by size or name on the Custom Clean,
  Duplicates, and Large Files tabs.
- **Command line** — `cleaner scan-custom <dir>`, `cleaner clean-system --yes`
  and friends, for scripting or Task Scheduler. `cleaner help` lists them all.

Safety, since that's the part that matters:

- Dry run is on out of the box — nothing is deleted until you turn it off
- Deletion goes to the recycle bin by default; permanent delete is opt-in
- Every clean asks for confirmation first
- Optional 3-pass secure erase for files you really want gone
- A protected-paths list that's never scanned or deleted, period
- A persistent log records every operation

Privacy: Cleaner makes **zero network calls**. No telemetry, no analytics, no
accounts. You can check — the source is right here.

## Install

**Installer (recommended):** grab `Cleaner-Setup-2.9.3.exe` from the
[Releases page](https://github.com/w0wzahh/cleaner/releases) and run it. It
installs per-user (no admin prompt), adds a Start Menu shortcut, and
registers a proper uninstaller.

**Portable zip:** grab `cleaner-v2.9.3-windows-x64.zip` instead, unzip
anywhere, run `cleaner.exe`.

## Uninstalling

Settings → Apps → Cleaner → Uninstall, or "Uninstall Cleaner" in the Start
Menu folder. It removes the whole program folder, deletes the scheduled
task if you registered one, and then asks if you want the data folder gone
too — say yes and there's genuinely nothing left.

## Using it

1. Pick a tool from the sidebar (Dashboard is a good first stop)
2. Choose a folder and tweak the filters
3. Hit **Scan** — this only previews matches
4. Look over the results, then hit **Clean**

Six themes are in the header bar — Midnight is the default — and switching
between them animates instead of snapping.

## Where your files live

Everything the app saves goes in one folder:

- **Installed:** next to the exe (`...\Programs\Cleaner\`) — the whole thing
  self-destructs on uninstall.
- **Portable:** next to wherever you put `cleaner.exe`.
- If the exe's folder is ever read-only, data moves to `%APPDATA%\Cleaner\`
  automatically.

Inside you'll find `cleaner_settings.json`, `cleaner_history.log`, a
`reports\` folder for exports, and a `README.txt` that explains each one.
The About tab has an "Open data folder" button if you ever want to look.

## Settings

| Field | What it does |
|---|---|
| `theme` | `Midnight`, `Dark`, `Light`, `Nord`, `Dracula`, or `Solarized` |
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
| `dupe_excludes` | Folders the duplicate finder skips |
| `custom_targets` | Your own entries in the System Cleaner list |
| `total_files_cleaned` / `total_space_freed` | Lifetime dashboard counters |

## Building it yourself

You'll need [Rust](https://rustup.rs/). Then:

```bash
cargo build --release
```

The binary lands at `target\release\cleaner.exe`. `cargo run` works for
development.

To build the installer you'll also need [Inno Setup 6](https://jrsoftware.org/isinfo.php),
then run `ISCC.exe installer.iss` — the setup lands in `dist\`.

## Changelog

See [CHANGELOG.md](CHANGELOG.md). Current version is **2.9.3**.

## License

MIT — see [LICENSE](LICENSE). Do whatever you want with it.

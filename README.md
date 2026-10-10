<div align="center">
  <img src="icon.png" alt="Cleaner logo" width="110" />

  <h1>Cleaner</h1>

  <p>
    <strong>A fast, safe system cleaner for Windows — written in Rust.</strong><br />
    Preview before you delete. Nothing is removed until you say so.
  </p>

  <p>
    <a href="https://github.com/w0wzahh/cleaner/releases/latest"><img src="https://img.shields.io/github/v/release/w0wzahh/cleaner?style=flat-square" alt="Latest release" /></a>
    <a href="LICENSE"><img src="https://img.shields.io/github/license/w0wzahh/cleaner?style=flat-square" alt="License: MIT" /></a>
    <a href="https://github.com/w0wzahh/cleaner/releases"><img src="https://img.shields.io/badge/platform-Windows-0078D6?style=flat-square&amp;logo=windows&amp;logoColor=white" alt="Platform: Windows" /></a>
    <a href="Cargo.toml"><img src="https://img.shields.io/badge/built%20with-Rust-dea584?style=flat-square&amp;logo=rust&amp;logoColor=white" alt="Built with Rust" /></a>
    <a href="https://github.com/w0wzahh/cleaner/releases"><img src="https://img.shields.io/github/downloads/w0wzahh/cleaner/total?style=flat-square" alt="Total downloads" /></a>
  </p>

  <p>
    <a href="https://w0wzahh.github.io/cleaner"><strong>Website</strong></a> ·
    <a href="https://github.com/w0wzahh/cleaner/releases/latest"><strong>Download</strong></a> ·
    <a href="https://w0wzahh.github.io/cleaner/docs.html"><strong>Docs</strong></a> ·
    <a href="https://github.com/w0wzahh/cleaner/issues"><strong>Report a bug</strong></a> ·
    <a href="https://ko-fi.com/w0wzahh"><strong>Ko-fi</strong></a>
  </p>
</div>

---

Cleaner digs through a folder — or the usual junk spots on your system — and
shows you exactly what's wasting space: duplicates, huge files, stale temp
data, empty folders. **Dry-run mode is on by default** and files go to the
recycle bin unless you say otherwise, so you can look before you leap.

## Features

- **Custom clean** — point it at any folder and filter by extension, glob
  pattern, age, or size. Full preview before anything is touched.
- **Duplicate finder** — groups files by size, fingerprints with **xxh3**
  (first 8 KB + last 4 KB + size), and only runs a full **BLAKE3** hash when
  it has to — in parallel across every CPU core. A 12,000-file folder with
  6,000 duplicate groups scans in well under a second.
- **Large file finder** — everything above a size you pick, sorted biggest
  first, with one-click reveal in Explorer.
- **System cleaner** — temp folders, app caches, browser caches (Chrome,
  Edge, Firefox), and the Windows thumbnail cache. You pick the targets —
  and can add your own.
- **Empty folder cleaner** — finds empty directories, including ones that
  only become empty once their children are removed.
- **Folder sizes & storage overview** — ranks subfolders by total size, plus
  per-drive usage bars so you can see where the space went.
- **Scheduled scans** — every hour, 6/12/24 hours, or weekly while the app
  is open, with optional auto-clean. Can also register a Windows Task
  Scheduler entry so scans run even when the app is closed.
- **System tray** — "To tray" hides the window while Cleaner keeps running
  its schedule in the background.
- **Sortable results & reports** — order by size or name, export a report of
  any scan.
- **Full command line** — everything the GUI can do, scriptable for Task
  Scheduler. See the [CLI reference](#cli).
- **Six animated themes** — Midnight (default), Dark, Light, Nord, Dracula,
  Solarized — switching animates instead of snapping.

## Safety by default

A cleaner deletes files, so every default is tuned to make that boring:

- Dry run is on out of the box — nothing is deleted until you turn it off
- Deletion goes to the recycle bin; permanent delete is strictly opt-in
- Every clean asks for confirmation first
- Optional 3-pass secure erase for files you really want gone
- A protected-paths list that's never scanned or deleted, period
- A persistent log records every operation

**Privacy:** Cleaner makes **zero network calls**. No telemetry, no
analytics, no accounts, no update pings. You can check — the source is
right here.

## Install

**Installer (recommended):** grab `Cleaner-Setup-<ver>.exe` from
[Releases](https://github.com/w0wzahh/cleaner/releases/latest) and run it.
Per-user install (no admin prompt), Start Menu shortcut, proper uninstaller.

**Portable zip:** grab `cleaner-v<ver>-windows-x64.zip`, unzip anywhere,
run `cleaner.exe`. Nothing is left behind when you delete the folder.

**Requirements:** Windows 10/11 (64-bit). No dependencies, no admin rights
needed.

**Uninstalling:** Settings → Apps → Cleaner → Uninstall. It removes the
program folder and any scheduled task, then asks if you want the data
folder gone too — say yes and nothing is left.

## Usage

1. Pick a tool from the sidebar (Dashboard is a good first stop)
2. Choose a folder and tweak the filters
3. Hit **Scan** — this only previews matches
4. Look over the results, then hit **Clean**

## CLI

Run `cleaner` with no arguments to open the GUI. `cleaner help` lists every
command. All `clean-*` commands preview only when dry run is on — pass
`--force` to delete for real.

| Scans | |
|---|---|
| `cleaner scan-system` | Scan the enabled system-clean targets |
| `cleaner scan-custom <dir>` | Scan a folder with filters |
| `cleaner scan-duplicates <dir>` | Find duplicate files |
| `cleaner scan-large <dir>` | Find large files |
| `cleaner scan-empty <dir>` | Find empty folders |
| `cleaner scan-folders <dir>` | Folder-size breakdown |

| Cleans | |
|---|---|
| `cleaner clean-system [--yes]` | Clean enabled system targets |
| `cleaner clean-custom <dir> [filters] [--yes]` | Clean a folder with filters |
| `cleaner clean-duplicates <dir> [--yes]` | Keeps the first file in each group |
| `cleaner clean-large <dir> [--min-mb N] [--yes]` | Clean files above a size threshold |
| `cleaner clean-empty <dir> [--yes]` | Remove empty folders |

| Flags | |
|---|---|
| `--ext a,b,c` | Extensions (default: tmp,log) |
| `--pattern <glob>` | Glob pattern, e.g. `"*.tmp"` |
| `--older-than <d>` | Only files older than N days |
| `--min-size <b>` / `--max-size <b>` | Size bounds in bytes |
| `--exclude <paths>` | Comma-separated directories to skip |
| `--recursive` / `--hidden` | Include subdirectories / hidden files |
| `--yes` | Skip the confirmation prompt |
| `--permanent` / `--secure` | Bypass recycle bin / 3-pass overwrite |
| `--force` | Delete for real even if dry run is enabled |
| `--minimized` | Launch the GUI parked in the tray |

```powershell
cleaner clean-custom "C:\Users\you\Downloads" --ext tmp,log --older-than 30 --recursive --yes
```

The full reference lives in the
[documentation](https://w0wzahh.github.io/cleaner/docs.html).

## Where your files live

Everything the app saves goes in one folder:

- **Installed:** next to the exe (`...\Programs\Cleaner\`)
- **Portable:** next to wherever you put `cleaner.exe`
- Falls back to `%APPDATA%\Cleaner\` if the exe's folder is read-only

Inside you'll find `cleaner_settings.json`, `cleaner_history.log`, and a
`reports\` folder for exports. The About tab has an "Open data folder"
button if you ever want to look.

## Settings

Saved to `cleaner_settings.json` in the data folder.

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

## Repository layout

```
src/           Rust app (egui) — the main application
src/cleaner/   Cleaner logic modules (JS) for the Electron port
main.js        Electron main process      } deepspace-cleaner:
preload.js     Electron preload bridge    } experimental Electron
renderer/      Electron UI (HTML/CSS/JS)  } port of the app
docs/          Project website (GitHub Pages)
installer.iss  Inno Setup script for the Windows installer
tests/         Rust test suite
```

## Building it yourself

You'll need [Rust](https://rustup.rs/). Then:

```bash
cargo build --release
```

The binary lands at `target\release\cleaner.exe`. `cargo run` works for
development, `cargo test` runs the suite.

To build the installer you'll also need
[Inno Setup 6](https://jrsoftware.org/isinfo.php), then run
`ISCC.exe installer.iss` — the setup lands in `dist\`.

The Electron port lives at the repo root and builds with
[Node.js](https://nodejs.org/):

```bash
npm install
npm start
```

## Contributing

Issues and pull requests are welcome on
[GitHub](https://github.com/w0wzahh/cleaner). By contributing you agree to
license your work under the project's [MIT license](LICENSE).

## Support

If Cleaner saved you some disk space, you can buy the author a coffee on
[Ko-fi](https://ko-fi.com/w0wzahh).

## Changelog

See [CHANGELOG.md](CHANGELOG.md). Current version is **2.12.3**.

## License

[MIT](LICENSE) — © 2026 0xEmre. Do whatever you want with it.

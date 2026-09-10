<div align="center">

# Cleaner

**A fast, modern, and safe system cleaning utility — built in Rust.**

<img src="https://img.shields.io/badge/version-2.4.0-blue.svg?style=flat-square" alt="version">
<img src="https://img.shields.io/badge/rust-1.70%2B-orange.svg?style=flat-square" alt="rust">
<img src="https://img.shields.io/badge/license-MIT-green.svg?style=flat-square" alt="license">
<img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg?style=flat-square" alt="platform">

Cleaner is a next-generation system maintenance tool that goes far beyond the outdated Windows Recycle Bin. It's fast, intelligent, and lightweight — written entirely in Rust with a sleek, modern GUI.

[Features](#features) &nbsp;•&nbsp; [Installation](#installation) &nbsp;•&nbsp; [Usage](#usage) &nbsp;•&nbsp; [Building](#building-from-source) &nbsp;•&nbsp; [Changelog](#changelog) &nbsp;•&nbsp; [License](#license)

</div>

---

## Features

<h3><img src="https://api.iconify.design/mdi/folder-search-outline.svg?color=%23818cf8" width="22" align="center">&nbsp; Custom File Cleaner</h3>

Scan any directory with fine-grained filters: extensions, glob patterns, age, file size, and excluded subdirectories. Preview every match before deleting.

<h3><img src="https://api.iconify.design/mdi/content-copy.svg?color=%23818cf8" width="22" align="center">&nbsp; Duplicate Finder</h3>

Locate duplicate files using a **two-stage hashing algorithm** (quick hash on the first 8 KB, then full SHA-256 only when needed). This makes the scan dramatically faster than naive solutions — no more waiting around.

<h3><img src="https://api.iconify.design/mdi/harddisk.svg?color=%23818cf8" width="22" align="center">&nbsp; Large File Finder</h3>

Find files above a configurable size threshold and reclaim gigabytes of disk space in seconds.

<h3><img src="https://api.iconify.design/mdi/broom.svg?color=%23818cf8" width="22" align="center">&nbsp; System Cleaner</h3>

Target common junk locations with one click:

- User cache
- System temp
- Local AppData temp
- Browser caches (Chrome, Edge, Firefox)
- Windows thumbnail cache

<h3><img src="https://api.iconify.design/mdi/folder-remove-outline.svg?color=%23818cf8" width="22" align="center">&nbsp; Empty Folder Cleaner</h3>

Detects and removes empty directories — including **cascade detection** (folders that become empty after their children are removed).

<h3><img src="https://api.iconify.design/mdi/shield-lock-outline.svg?color=%23818cf8" width="22" align="center">&nbsp; Secure Delete</h3>

Optional 3-pass overwrite with cryptographically random data before deletion. Irrecoverable.

<h3><img src="https://api.iconify.design/mdi/palette-outline.svg?color=%23818cf8" width="22" align="center">&nbsp; Five Themes with Animated Transitions</h3>

Dark, Light, Nord, Dracula, and Solarized. Switching themes smoothly interpolates between them — no jarring flicker.

<h3><img src="https://api.iconify.design/mdi/chart-donut.svg?color=%23818cf8" width="22" align="center">&nbsp; Storage Overview</h3>

See per-drive usage with visual progress bars.

<h3><img src="https://api.iconify.design/mdi/file-export-outline.svg?color=%23818cf8" width="22" align="center">&nbsp; Exportable Reports</h3>

Save detailed reports of any scan (custom files, duplicates, large files, empty folders) as timestamped text files.

<h3><img src="https://api.iconify.design/mdi/shield-check-outline.svg?color=%23818cf8" width="22" align="center">&nbsp; Safety First</h3>

- **Dry run mode** by default — nothing gets deleted until you say so
- **Move to recycle bin** option (safer than permanent deletion)
- **Confirmation dialogs** before destructive actions
- **Persistent history log** of every operation

---

## Screenshots

> Screenshots coming soon. In the meantime, build and run the app yourself — it's fully self-contained.

---

## Installation

### Option 1 — Download a prebuilt binary

Head over to the [Releases page](https://github.com/w0wzahh/cleaner/releases) and download the latest archive for your platform.

- **Windows:** `cleaner-vX.X.X-windows-x64.zip` — extract and run `cleaner.exe`
- **macOS & Linux:** Build from source (see below)

### Option 2 — Build from source

You'll need the [Rust toolchain](https://rustup.rs/) installed.

```bash
git clone https://github.com/w0wzahh/cleaner.git
cd cleaner
cargo build --release
```

The binary will be at `target/release/cleaner` (or `cleaner.exe` on Windows).

---

## Usage

Run the compiled binary:

```bash
./cleaner
```

The GUI opens with a tabbed interface. Every tab follows the same pattern:

1. **Pick a directory** (via the Browse button)
2. **Configure your filters**
3. **Click Scan** to preview matches
4. **Click Clean** to remove them (with confirmation)

### Recommended workflow

- First run: leave **Dry run** checked so you can see exactly what would be deleted
- Once you trust the scan results: uncheck Dry run and let it clean
- Keep **Move to recycle bin** enabled unless you specifically want permanent deletion
- Enable **Secure Delete** only for sensitive files you need to shred

---

## Configuration

Settings are stored in `cleaner_settings.json` next to the executable. It's automatically created on first run.

| Field | Description |
|---|---|
| `theme` | One of `Dark`, `Light`, `Nord`, `Dracula`, `Solarized` |
| `use_trash` | Move deleted files to recycle bin |
| `dry_run` | Preview only, don't delete anything |
| `recursive` | Scan subdirectories recursively |
| `include_hidden` | Include dotfiles and hidden folders |
| `confirm_clean` | Show confirmation dialog before deletion |
| `secure_delete` | Enable 3-pass shredder |
| `default_dir` | Default starting directory |
| `github_url` | URL opened by the "Open" button in About |
| `log_file` | Path to the persistent history log |

---

## Building from Source

### Prerequisites

- Rust 1.70 or newer
- A working C toolchain (for Windows: MSVC; for Linux: `build-essential`; for macOS: Xcode command-line tools)

### Build

```bash
cargo build --release
```

### Run in development

```bash
cargo run
```

### Package a release (Windows)

```powershell
cargo build --release
Compress-Archive -Path target\release\cleaner.exe -DestinationPath cleaner-windows-x64.zip -Force
```

---

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for the full version history.

**Latest — v2.4.0**

- Five selectable themes with animated transitions
- Procedurally-generated app icon
- Empty Folder cleaner (cascade-aware)
- Secure Delete (3-pass overwrite)
- Export Report button on all scan tabs

---

## Contributing

Contributions are welcome. If you have a feature idea, bug report, or PR — open an issue or submit a pull request.

1. Fork the repo
2. Create a branch: `git checkout -b feature/my-feature`
3. Commit your changes: `git commit -m "Add my feature"`
4. Push: `git push origin feature/my-feature`
5. Open a Pull Request

---

## License

This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for details.

---

<div align="center">

**Made with Rust**

[Report a Bug](https://github.com/w0wzahh/cleaner/issues) &nbsp;•&nbsp; [Request a Feature](https://github.com/w0wzahh/cleaner/issues) &nbsp;•&nbsp; [Releases](https://github.com/w0wzahh/cleaner/releases)

</div>
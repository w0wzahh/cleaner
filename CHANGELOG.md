# Changelog

Every release, with what changed and why. If you're wondering what a specific
version added or fixed, this is the place to look.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) and
versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

Things planned for the next release. Nothing here is final yet.

- Command-line mode for scripting and headless cleanup
- Scheduled scans (run on a timer in the background)
- Custom system-clean targets (add your own paths to the list)
- Per-file selection in the Custom Clean tab

---

## [2.4.1] — 2026-09-12

UI overhaul and bug-fix release.

### Added
- **New UI**: left sidebar navigation, header bar, card-based layouts, and
  per-theme accent colors with rounded widgets.
- **Dashboard**: stat cards, safety toggles, quick actions, and a recent
  activity view (the log was previously written but never shown).
- **Large Files**: checkboxes per result, select-all/clear, and a clean action.
- **Storage**: disk list is now cached with a manual Refresh button instead of
  re-enumerating drives every frame.

### Fixed
- **Crash after cleaning**: the app panicked while parsing the clean summary
  string (`start > end` slice). Clean results now use a structured message.
- **Cancel now works** for custom, large-file, system, and empty-folder scans.
- Duplicate quick-hash only reads the first 8 KB instead of whole files;
  full hashes are streamed instead of loaded into memory.
- Zero-byte files are no longer reported as duplicates.
- Empty-folder removal now respects dry-run and recycle-bin settings.
- The duplicate "wasted space" total now tracks checkbox selection.
- The `default_dir` setting is now actually honored.
- "Reveal settings file" now opens the settings file (was opening the log).

---

## [2.4.0] — 2025-02-01

The "make it feel like a real product" release.

### Added
- **Five themes**: Dark, Light, Nord, Dracula, and Solarized. Pick one from the
  combo box in the top toolbar.
- **Animated theme transitions**. Switching themes now interpolates colors over
  0.35 seconds with an ease-out curve instead of snapping instantly.
- **Procedurally-generated app icon**. No external image file — it's drawn at
  startup, so the binary stays self-contained.
- **Empty Folder Cleaner tab**. Walks the tree bottom-up and removes any folder
  that contains nothing. Handles cascades properly — if `a/b/c` is empty,
  deleting `c` might make `b` empty, which then makes `a` empty.
- **Secure Delete**. Optional 3-pass overwrite with random data before removal.
  Off by default. Don't turn it on for files you can't afford to lose.
- **Export Report** button on the Custom, Duplicates, Large Files, and Empty
  Folders tabs. Dumps a timestamped `.txt` file with everything that matched.

### Changed
- Theme selection moved into the top toolbar as a dropdown. It used to be a
  checkbox that only toggled dark/light.
- All confirmation dialogs now go through a single code path. Less duplicated
  code, easier to add new confirmations later.
- Settings schema changed to store the theme as an enum instead of a boolean.
  If you had `theme_dark: true` saved, it'll be ignored and defaults to Dark.

### Fixed
- The version string in the About tab now actually matches `Cargo.toml`. It was
  hardcoded and drifted out of sync.
- Removed a couple of compiler warnings that had been hanging around since 2.2.

---

## [2.3.0] — 2025-01-25

### Added
- **"Check for Updates"** button in the About tab. It opens the GitHub releases
  page in your browser, where you can grab the latest build.
- Comments throughout the source got a whole lot more personality. If you're
  reading the code, you'll know.

### Changed
- Version bumped to 2.3.0 everywhere. No more lies in the build output.

---

## [2.2.0] — 2025-01-20

### Added
- **Spinner animation** in the status bar while a scan or clean is running. Small
  thing, but it makes the app feel alive.
- **Two-stage duplicate scanning**. The finder now quick-hashes the first 8 KB
  of each file (plus its size) to filter out obvious non-matches, then runs a
  full SHA-256 only on files that passed the quick check. On large folders this
  is a massive speedup — minutes reduced to seconds.

### Changed
- Removed the unused `age_days` field from the internal file struct. It was
  being tracked but never actually shown to the user.

### Fixed
- Fixed two compiler warnings: an unnecessary `mut` and a dead-code warning on
  the file struct.

---

## [2.1.0] — 2025-01-15

### Added
- **Storage Overview tab**. Shows per-drive usage with progress bars, using the
  `sysinfo` crate.
- **Changelog tab** inside the app. Displays the embedded changelog so you don't
  have to leave the app to see what's new.
- **GitHub link field** in Settings and About, with a button that opens it in
  your browser.
- **Persistent history log**. Every action gets appended to
  `cleaner_history.log` next to the executable. Useful for auditing.
- **`open` crate** dependency, used for opening URLs.

### Changed
- Improved the UI: more tabs, better spacing, clearer sections.
- Duplicate scanning now sends progress updates during the hashing phase instead
  of silently chugging along.

### Fixed
- Duplicate scanning used to freeze at 0% during hashing. It now reports
  progress the whole way through.
- Fixed an unused-variable warning in the message polling loop.

---

## [2.0.0] — 2025-01-10

The first real release. Rewrote the whole thing from a basic command-line tool
into a proper GUI application.

### Added
- **Dashboard, Custom Clean, Duplicates, Large Files, System Cleaner, and About
  tabs**. The full feature set you see today started here.
- **Safe deletion** to the recycle bin, so a misclick doesn't nuke your files.
- **Dry run mode** — the default. Shows you what would be deleted without
  actually deleting it.
- **Confirmation dialogs** before any destructive action.
- **Dark and light themes**.
- **Configuration persistence** via `cleaner_settings.json`.

### Changed
- Complete rewrite of the previous simple file cleaner into a GUI application.

---

## [1.0.0] — 2025-01-05

The prototype. A command-line cleaner written in Rust. Never released publicly,
but it's where all of this started.

### Added
- Basic file cleaning by extension and age.
- Dry run mode.
- Recursive directory scanning.
- Colored terminal output.

---

[Unreleased]: https://github.com/w0wzahh/cleaner/compare/v2.4.0...HEAD
[2.4.0]: https://github.com/w0wzahh/cleaner/compare/v2.3.0...v2.4.0
[2.3.0]: https://github.com/w0wzahh/cleaner/compare/v2.2.0...v2.3.0
[2.2.0]: https://github.com/w0wzahh/cleaner/compare/v2.1.0...v2.2.0
[2.1.0]: https://github.com/w0wzahh/cleaner/compare/v2.0.0...v2.1.0
[2.0.0]: https://github.com/w0wzahh/cleaner/releases/tag/v2.0.0
[1.0.0]: https://github.com/w0wzahh/cleaner/releases/tag/v1.0.0
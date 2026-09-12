# Changelog

Every release, with what changed and why. If you're wondering what a specific
version added or fixed, this is the place to look.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) and
versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

Things planned for the next release. Nothing here is final yet.

---

## [2.9.1] — 2026-09-12

### Fixed
- **Tray restore** — "Show Cleaner" / double-click now reliably brings the
  window back. Tray events are handled by a dedicated watcher thread instead
  of the UI loop, which stops running while the window is hidden.
- **Stray console window** — the exe is now a proper GUI-subsystem binary;
  launching it no longer opens an empty terminal. CLI mode re-attaches to
  the parent console, so `cleaner help` etc. still print normally.

---

## [2.9.0] — 2026-09-12

### Added
- **Sort options** on Custom Clean, Duplicates, and Large Files results —
  size or name, either direction, via a combo next to the filter box.
- **Duplicates exclude list** — folders the duplicate finder skips (one per
  line), editable from a collapsible in its Scan setup card.
- **"Run when the app is closed"** — the Scheduler card can register a
  Windows Task Scheduler entry that runs the same schedule headlessly.
  The uninstaller removes the task automatically.
- **System tray** — a tray icon with Show/Quit menu, plus a "To tray"
  header button that hides the window while the app keeps running.

### Fixed
- Duplicate scans now honor protected paths (they were only enforced in
  custom scans and cleans before).

---

## [2.8.0] — 2026-09-12

The "proper app" release — installer, organized data folder, and a privacy
stance you can verify.

### Added
- **Windows installer** (`Cleaner-Setup-2.8.0.exe`) built with Inno Setup:
  per-user install to `%LOCALAPPDATA%\Programs\Cleaner` with no admin prompt,
  Start Menu shortcut, optional desktop icon, and a registered uninstaller in
  Windows Settings. Uninstall removes the whole program folder; it also offers
  to delete the data folder if one exists elsewhere.
- **Organized data folder**: when the exe can't write next to itself (e.g. an
  admin install to `Program Files`), settings, history, and reports move to
  `%APPDATA%\Cleaner\`. A `README.txt` inside explains every file.
- Exported reports now go to a `reports\` subfolder of the data dir instead of
  the working directory.
- **Getting-started card** on the dashboard for first run — explains the
  scan → review → clean flow and the dry-run default. Dismissible.
- **Privacy card** in About: states plainly that nothing leaves the PC, and
  shows where your data folder is.
- Tooltips on the main scan/clean buttons, an "Open data folder" button, and
  a sensible minimum window size.
- **Exe icon embedded**: `cleaner.exe` shows the broom in Explorer, on the
  taskbar, and in shortcuts.

### Changed
- The portable zip still keeps settings next to the exe — nothing moves
  unless the exe's folder is read-only.

---

## [2.7.1] — 2026-09-12

### Added
- **Results filter box** on Custom Clean, Duplicates, and Large Files — type
  a substring to narrow down the list.
- Proper icons for the status indicator (eye = dry run, check = live).

### Changed
- **App icon background is now fully transparent** — just the broom and
  sparkles, no tile, so it sits cleanly on any taskbar.
- Sidebar/tile icons redrawn as filled silhouettes — crisper and more
  professional than the first hand-drawn set.

---

## [2.7.0] — 2026-09-12

New app icon and scheduled scans.

### Added
- **Scheduled scans**: the Scheduler card on the dashboard runs a scan every
  1/6/12/24 hours or weekly — on System junk or your Custom Clean folder —
  while the app is open. Optional auto-clean follows the scan (skipped when
  dry run is on).
- **Real app icon**: the window and taskbar icon is now the broom artwork,
  with its background tinted to a light shade of the active theme's accent.
  Switching themes recolors the icon live.

### Changed
- Sidebar and dashboard tile icons are hand-drawn line glyphs instead of
  emoji — they render identically on every machine (some emoji were showing
  up as empty boxes).

---

## [2.6.0] — 2026-09-12

The "make it feel premium" release — a visual overhaul inspired by modern
cleaner apps.

### Added
- **Midnight theme**: deep purple surfaces with a magenta accent and a violet
  gradient. It's the new default — old themes are still there in the picker.
- **Dashboard hero**: "Welcome to Cleaner" next to a big glowing ring button.
  Click it to start a smart scan; while anything is running, the ring becomes
  a live progress arc (and a spinner when progress is indeterminate).
- **Tool tiles**: icon cards on the dashboard that jump straight to each tool.
- **Activity chart**: a little bar chart of files cleaned per day over the
  last week, fed by real clean history persisted in settings.
- **Sidebar icons** and a "Last clean" timestamp on the dashboard.

### Changed
- Quick actions moved beside the activity chart; stats row stays on the
  dashboard under the hero.

---

## [2.5.0] — 2026-09-12

The "more control" release.

### Added
- **Per-file selection**: every match in Custom Clean gets a checkbox, with
  select-all / clear buttons. Clean only touches what you checked.
- **Custom system targets**: add your own folders to the System Cleaner list
  (persisted in settings, removable with one click).
- **Protected paths**: a never-touch list on the Dashboard. Files and folders
  under a protected path are skipped during scans *and* cleans.
- **Persistent stats**: files cleaned and space freed now survive restarts.
- **Folder Sizes tab**: ranks top-level subfolders by size with share bars.
- **File-type breakdown**: Custom Clean results show which extensions account
  for the space.
- **Scan presets**: one-click recipes — Temp & logs, Old files (30d+),
  Big media (50MB+), Images, Old Downloads.
- **CLI mode**: `cleaner scan-custom <dir>`, `clean-system --yes`, etc.
  Run `cleaner help` for the full list — enables Task Scheduler automation.

### Changed
- Custom Clean deletes only the selected files, not every match.

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

[Unreleased]: https://github.com/w0wzahh/cleaner/compare/v2.9.1...HEAD
[2.9.1]: https://github.com/w0wzahh/cleaner/compare/v2.9.0...v2.9.1
[2.9.0]: https://github.com/w0wzahh/cleaner/compare/v2.8.0...v2.9.0
[2.8.0]: https://github.com/w0wzahh/cleaner/compare/v2.7.1...v2.8.0
[2.7.1]: https://github.com/w0wzahh/cleaner/compare/v2.7.0...v2.7.1
[2.7.0]: https://github.com/w0wzahh/cleaner/compare/v2.6.0...v2.7.0
[2.6.0]: https://github.com/w0wzahh/cleaner/compare/v2.5.0...v2.6.0
[2.5.0]: https://github.com/w0wzahh/cleaner/compare/v2.4.1...v2.5.0
[2.4.1]: https://github.com/w0wzahh/cleaner/compare/v2.4.0...v2.4.1
[2.4.0]: https://github.com/w0wzahh/cleaner/compare/v2.3.0...v2.4.0
[2.3.0]: https://github.com/w0wzahh/cleaner/compare/v2.2.0...v2.3.0
[2.2.0]: https://github.com/w0wzahh/cleaner/compare/v2.1.0...v2.2.0
[2.1.0]: https://github.com/w0wzahh/cleaner/compare/v2.0.0...v2.1.0
[2.0.0]: https://github.com/w0wzahh/cleaner/releases/tag/v2.0.0
[1.0.0]: https://github.com/w0wzahh/cleaner/releases/tag/v1.0.0
//! Application state, event loop wiring, and the egui UI for Cleaner.
//!
//! Layout: left sidebar navigation, a header bar with the current page title and
//! global controls, a bottom status bar, and card-based content per tab.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use chrono::Local;
use eframe::egui;
use rfd::FileDialog;
use std::io::Write;

use crate::helpers;
use crate::models::*;
use crate::settings;
use crate::themes;
use crate::workers;

// -----------------------------------------------------------------------------
// shared UI helpers
// -----------------------------------------------------------------------------

fn accent(ui: &egui::Ui) -> egui::Color32 {
    ui.visuals().selection.bg_fill
}

fn danger_color() -> egui::Color32 {
    egui::Color32::from_rgb(0xC0, 0x39, 0x2B)
}

fn warn_color() -> egui::Color32 {
    egui::Color32::from_rgb(0xE0, 0x9A, 0x2E)
}

/// Theme-aware card container; stretches to the available width.
fn card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(&ui.style())
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(egui::Margin::same(14.0))
        .rounding(egui::Rounding::same(8.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if !title.is_empty() {
                ui.label(egui::RichText::new(title).strong().size(15.0));
                ui.add_space(6.0);
            }
            add_contents(ui);
        });
}

/// Dashboard stat tile.
fn stat_card(ui: &mut egui::Ui, label: &str, value: String, note: &str) {
    egui::Frame::group(&ui.style())
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(egui::Margin::same(14.0))
        .rounding(egui::Rounding::same(8.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(egui::RichText::new(label).weak().small());
            ui.label(egui::RichText::new(value).size(18.0).strong());
            if !note.is_empty() {
                ui.label(egui::RichText::new(note).weak().small());
            }
        });
}

fn primary_button(ui: &mut egui::Ui, enabled: bool, text: impl Into<String>) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(
            egui::RichText::new(text.into())
                .strong()
                .color(egui::Color32::WHITE),
        )
        .fill(accent(ui)),
    )
}

fn danger_button(ui: &mut egui::Ui, enabled: bool, text: impl Into<String>) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(
            egui::RichText::new(text.into())
                .strong()
                .color(egui::Color32::WHITE),
        )
        .fill(danger_color()),
    )
}

/// Sidebar navigation entry; returns true when clicked.
fn nav_item(ui: &mut egui::Ui, current: Tab, target: Tab, label: &str) -> bool {
    let w = ui.available_width();
    ui.add_sized(
        [w, 32.0],
        egui::SelectableLabel::new(
            current == target,
            egui::RichText::new(format!("  {}", label)).size(13.5),
        ),
    )
    .clicked()
}

fn nav_section(ui: &mut egui::Ui, label: &str) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(format!("  {}", label))
            .weak()
            .small()
            .size(10.0),
    );
    ui.add_space(2.0);
}

/// Directory row: label, growing text field, Browse button.
fn dir_picker(ui: &mut egui::Ui, path: &mut String) {
    ui.horizontal(|ui| {
        ui.label("Directory");
        let browse_w = 76.0;
        let w = (ui.available_width() - browse_w).max(80.0);
        ui.add_sized(
            [w, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(path).hint_text("Choose a folder to scan"),
        );
        if ui.button("Browse…").clicked() {
            if let Some(p) = FileDialog::new().pick_folder() {
                *path = p.display().to_string();
            }
        }
    });
}

fn empty_state(ui: &mut egui::Ui, text: &str) {
    ui.add_space(14.0);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(text).weak());
    });
    ui.add_space(8.0);
}

/// Scrollable list of matched files with right-aligned sizes.
fn matched_file_rows(ui: &mut egui::Ui, files: &[MatchedFile], id: &str) {
    egui::ScrollArea::vertical()
        .id_source(id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if files.is_empty() {
                empty_state(ui, "Nothing here yet — run a scan.");
                return;
            }
            for f in files {
                ui.horizontal(|ui| {
                    ui.monospace(f.path.display().to_string());
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.label(
                                egui::RichText::new(helpers::human_size(f.size)).weak(),
                            );
                        },
                    );
                });
            }
        });
}

/// Open a path in the system file manager. Best-effort.
fn reveal_in_explorer(path: &Path) {
    let _ = open::that(path);
}

// -----------------------------------------------------------------------------
// app state
// -----------------------------------------------------------------------------

pub struct CleanerApp {
    pub tab: Tab,
    pub settings: settings::Settings,
    pub scanning: bool,
    pub cleaning: bool,
    pub cancel_flag: Arc<AtomicBool>,
    pub progress: f32,
    pub log: Vec<String>,
    pub rx: Option<mpsc::Receiver<workers::WorkerMessage>>,
    pub status: String,
    pub status_toast: u64,
    pub confirm_action: Option<ConfirmAction>,
    pub confirm_title: String,
    pub confirm_body: String,
    pub theme_anim: themes::ThemeAnim,
    pub pending_theme_change: Option<themes::Theme>,
    pub initial_theme_applied: bool,

    pub custom: CustomCleanerState,
    pub duplicates: DuplicateState,
    pub large_files: LargeFilesState,
    pub system: SystemCleanerState,
    pub empty_folders: EmptyFoldersState,

    pub storage_disks: Vec<DiskEntry>,
    pub storage_loaded: bool,

    pub total_files_cleaned: u64,
    pub total_space_freed: u64,
    pub last_scan_summary: String,
}

impl Default for CleanerApp {
    fn default() -> Self {
        let settings = settings::Settings::load();
        let initial_visuals = settings.theme.visuals();
        let mut app = Self {
            tab: Tab::Dashboard,
            settings,
            scanning: false,
            cleaning: false,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            progress: 0.0,
            log: Vec::new(),
            rx: None,
            status: "Ready".to_string(),
            status_toast: 0,
            confirm_action: None,
            confirm_title: "Confirm action".to_string(),
            confirm_body: String::new(),
            theme_anim: themes::ThemeAnim::new(initial_visuals),
            initial_theme_applied: false,
            pending_theme_change: None,
            custom: CustomCleanerState::default(),
            duplicates: DuplicateState::default(),
            large_files: LargeFilesState::default(),
            system: SystemCleanerState::default(),
            empty_folders: EmptyFoldersState::default(),
            storage_disks: Vec::new(),
            storage_loaded: false,
            total_files_cleaned: 0,
            total_space_freed: 0,
            last_scan_summary: "No scan yet".to_string(),
        };
        // Honor the configured default directory instead of the binary dir.
        if !app.settings.default_dir.is_empty() {
            let d = app.settings.default_dir.clone();
            app.custom.dir_path = d.clone();
            app.duplicates.dir_path = d.clone();
            app.large_files.dir_path = d.clone();
            app.empty_folders.dir_path = d;
        }
        app
    }
}

// -----------------------------------------------------------------------------
// logging + status
// -----------------------------------------------------------------------------

impl CleanerApp {
    pub fn add_log(&mut self, msg: &str) {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        let line = format!("[{}] {}", timestamp, msg);
        self.log.push(line.clone());
        if self.log.len() > 1000 {
            self.log.remove(0);
        }
        if self.status_toast == 0 {
            self.status = msg.to_string();
            self.status_toast = 60;
        }
        if let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.settings.log_path())
        {
            let _ = writeln!(file, "{}", line);
        }
    }
}

// -----------------------------------------------------------------------------
// theme animation
// -----------------------------------------------------------------------------

impl CleanerApp {
    pub fn update_theme_animation(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if self.theme_anim.active {
            let t = ((now - self.theme_anim.start) / self.theme_anim.duration)
                .clamp(0.0, 1.0) as f32;
            let et = themes::ease_out_cubic(t);
            let v = themes::lerp_visuals(&self.theme_anim.from, &self.theme_anim.to, et);
            ctx.set_visuals(v);
            if t >= 1.0 {
                self.theme_anim.active = false;
                ctx.set_visuals(self.theme_anim.to.clone());
            } else {
                ctx.request_repaint();
            }
        } else {
            ctx.set_visuals(self.theme_anim.to.clone());
        }
    }

    pub fn change_theme(&mut self, new_theme: themes::Theme, ctx: &egui::Context) {
        if new_theme == self.settings.theme {
            return;
        }
        let now = ctx.input(|i| i.time);
        let from = if self.theme_anim.active {
            let t = ((now - self.theme_anim.start) / self.theme_anim.duration)
                .clamp(0.0, 1.0) as f32;
            themes::lerp_visuals(
                &self.theme_anim.from,
                &self.theme_anim.to,
                themes::ease_out_cubic(t),
            )
        } else {
            self.theme_anim.to.clone()
        };
        let to = new_theme.visuals();
        self.theme_anim.start(from, to, now);
        self.settings.theme = new_theme;
        self.settings.save();
        self.add_log(&format!("Theme changed to {}", new_theme.label()));
    }
}

// -----------------------------------------------------------------------------
// worker starts
// -----------------------------------------------------------------------------

impl CleanerApp {
    fn busy(&self) -> bool {
        self.scanning || self.cleaning
    }

    pub fn start_custom_scan(&mut self) {
        if self.busy() {
            return;
        }
        self.settings.save();
        self.scanning = true;
        self.progress = 0.0;
        self.custom.matched_files.clear();
        self.custom.total_matched_size = 0;
        self.log.clear();
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let dir = self.custom.dir_path.clone();
        let ext = self.custom.extensions.clone();
        let days = self.custom.older_than_days;
        let min = self.custom.min_size_bytes;
        let max = self.custom.max_size_bytes;
        let pat = self.custom.pattern.clone();
        let excl = self.custom.exclude_dirs.clone();
        let rec = self.settings.recursive;
        let hidden = self.settings.include_hidden;
        self.add_log("Starting custom scan...");
        thread::spawn(move || {
            workers::custom_scan_worker(
                dir, ext, days, min, max, pat, excl, rec, hidden, cancel, tx,
            )
        });
        self.rx = Some(rx);
        self.status = "Scanning...".to_string();
        self.status_toast = 0;
    }

    pub fn start_duplicates_scan(&mut self) {
        if self.busy() {
            return;
        }
        self.settings.save();
        self.scanning = true;
        self.progress = 0.0;
        self.duplicates.groups.clear();
        self.duplicates.selected_files.clear();
        self.duplicates.total_wasted = 0;
        self.log.clear();
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let dir = self.duplicates.dir_path.clone();
        self.add_log("Starting duplicate scan...");
        thread::spawn(move || workers::duplicates_worker(dir, cancel, tx));
        self.rx = Some(rx);
        self.status = "Scanning duplicates...".to_string();
        self.status_toast = 0;
    }

    pub fn start_large_files_scan(&mut self) {
        if self.busy() {
            return;
        }
        self.settings.save();
        self.scanning = true;
        self.progress = 0.0;
        self.large_files.files.clear();
        self.large_files.selected.clear();
        self.large_files.total_size = 0;
        self.log.clear();
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let dir = self.large_files.dir_path.clone();
        let threshold = self.large_files.threshold_mb;
        self.add_log("Starting large file scan...");
        thread::spawn(move || workers::large_files_worker(dir, threshold, cancel, tx));
        self.rx = Some(rx);
        self.status = "Scanning large files...".to_string();
        self.status_toast = 0;
    }

    pub fn start_system_scan(&mut self) {
        if self.busy() {
            return;
        }
        self.settings.save();
        self.scanning = true;
        self.progress = 0.0;
        self.system.matched_files.clear();
        self.system.total_matched_size = 0;
        self.log.clear();
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let targets = self.system.targets.clone();
        self.add_log("Starting system scan...");
        thread::spawn(move || workers::system_scan_worker(targets, cancel, tx));
        self.rx = Some(rx);
        self.status = "Scanning system...".to_string();
        self.status_toast = 0;
    }

    pub fn start_empty_folders_scan(&mut self) {
        if self.busy() {
            return;
        }
        self.settings.save();
        self.scanning = true;
        self.progress = 0.0;
        self.empty_folders.folders.clear();
        self.log.clear();
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let dir = self.empty_folders.dir_path.clone();
        self.add_log("Starting empty folder scan...");
        thread::spawn(move || workers::empty_folders_worker(dir, cancel, tx));
        self.rx = Some(rx);
        self.status = "Scanning for empty folders...".to_string();
        self.status_toast = 0;
    }

    pub fn start_clean_selected(&mut self) {
        if self.busy() {
            return;
        }
        let files: Vec<MatchedFile> = match self.tab {
            Tab::CustomClean => self.custom.matched_files.clone(),
            Tab::SystemCleaner => self.system.matched_files.clone(),
            Tab::LargeFiles => self
                .large_files
                .files
                .iter()
                .filter(|f| self.large_files.selected.contains(&f.path))
                .map(|f| MatchedFile {
                    path: f.path.clone(),
                    size: f.size,
                })
                .collect(),
            _ => Vec::new(),
        };
        if files.is_empty() {
            return;
        }
        self.cleaning = true;
        self.progress = 0.0;
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let use_trash = self.settings.use_trash;
        let dry_run = self.settings.dry_run;
        let secure_delete = self.settings.secure_delete;
        self.add_log("Starting cleaning...");
        thread::spawn(move || {
            workers::clean_files(files, use_trash, dry_run, secure_delete, cancel, tx)
        });
        self.rx = Some(rx);
        self.status = "Cleaning...".to_string();
        self.status_toast = 0;
    }

    pub fn start_clean_duplicates(&mut self) {
        if self.busy() {
            return;
        }
        let files = self.duplicates.selected_files.clone();
        if files.is_empty() {
            return;
        }
        self.cleaning = true;
        self.progress = 0.0;
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let use_trash = self.settings.use_trash;
        let dry_run = self.settings.dry_run;
        let secure_delete = self.settings.secure_delete;
        let matched: Vec<MatchedFile> = files
            .into_iter()
            .map(|p| {
                let size = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                MatchedFile { path: p, size }
            })
            .collect();
        self.add_log("Starting duplicate cleanup...");
        thread::spawn(move || {
            workers::clean_files(matched, use_trash, dry_run, secure_delete, cancel, tx)
        });
        self.rx = Some(rx);
        self.status = "Cleaning duplicates...".to_string();
        self.status_toast = 0;
    }

    pub fn start_clean_empty_folders(&mut self) {
        if self.busy() {
            return;
        }
        let folders = self.empty_folders.folders.clone();
        if folders.is_empty() {
            return;
        }
        self.cleaning = true;
        self.progress = 0.0;
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let dry_run = self.settings.dry_run;
        let use_trash = self.settings.use_trash;
        self.add_log("Removing empty folders...");
        thread::spawn(move || {
            workers::clean_folders(folders, dry_run, use_trash, cancel, tx)
        });
        self.rx = Some(rx);
        self.status = "Removing empty folders...".to_string();
        self.status_toast = 0;
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    pub fn handle_confirm(&mut self) {
        if let Some(action) = self.confirm_action.take() {
            match action {
                ConfirmAction::CleanFiles => self.start_clean_selected(),
                ConfirmAction::CleanDuplicates => self.start_clean_duplicates(),
                ConfirmAction::CleanEmptyFolders => self.start_clean_empty_folders(),
            }
        }
    }

    /// Wasted space based on which duplicates are actually checked.
    fn dup_wasted(&self) -> u64 {
        self.duplicates
            .groups
            .iter()
            .map(|g| {
                g.size
                    * g.files
                        .iter()
                        .filter(|f| self.duplicates.selected_files.contains(f))
                        .count() as u64
            })
            .sum()
    }

    /// Drop results that were just deleted so the UI doesn't offer them twice.
    fn prune_after_clean(&mut self) {
        match self.tab {
            Tab::CustomClean => {
                self.custom.matched_files.clear();
                self.custom.total_matched_size = 0;
            }
            Tab::SystemCleaner => {
                self.system.matched_files.clear();
                self.system.total_matched_size = 0;
            }
            Tab::LargeFiles => {
                let sel = std::mem::take(&mut self.large_files.selected);
                self.large_files.files.retain(|f| !sel.contains(&f.path));
                self.large_files.total_size =
                    self.large_files.files.iter().map(|f| f.size).sum();
            }
            Tab::Duplicates => {
                let sel = std::mem::take(&mut self.duplicates.selected_files);
                for g in &mut self.duplicates.groups {
                    g.files.retain(|f| !sel.contains(f));
                }
                self.duplicates.groups.retain(|g| g.files.len() > 1);
                self.duplicates.total_wasted = self.dup_wasted();
            }
            Tab::EmptyFolders => {
                self.empty_folders.folders.clear();
            }
            _ => {}
        }
    }

    pub fn poll_messages(&mut self, ctx: &egui::Context) {
        if let Some(rx) = self.rx.take() {
            let mut still_active = true;
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    workers::WorkerMessage::Log(s) => self.add_log(&s),
                    workers::WorkerMessage::Progress(p) => self.progress = p,
                    workers::WorkerMessage::CustomMatched(files) => {
                        self.custom.total_matched_size =
                            files.iter().map(|f| f.size).sum();
                        self.custom.matched_files = files;
                    }
                    workers::WorkerMessage::Duplicates(groups) => {
                        let mut selected = Vec::new();
                        for group in &groups {
                            for file in group.files.iter().skip(1) {
                                selected.push(file.clone());
                            }
                        }
                        self.duplicates.selected_files = selected;
                        self.duplicates.groups = groups;
                        self.duplicates.total_wasted = self.dup_wasted();
                    }
                    workers::WorkerMessage::LargeFiles(files) => {
                        self.large_files.total_size =
                            files.iter().map(|f| f.size).sum();
                        self.large_files.files = files;
                        self.large_files.selected.clear();
                    }
                    workers::WorkerMessage::SystemMatched(files) => {
                        self.system.total_matched_size =
                            files.iter().map(|f| f.size).sum();
                        self.system.matched_files = files;
                    }
                    workers::WorkerMessage::EmptyFolders(folders) => {
                        self.empty_folders.folders = folders;
                    }
                    workers::WorkerMessage::CleanStats { deleted, freed } => {
                        self.total_files_cleaned += deleted;
                        self.total_space_freed += freed;
                    }
                    workers::WorkerMessage::Done { summary } => {
                        let was_cleaning = self.cleaning;
                        self.add_log(&summary);
                        self.status = if self.scanning {
                            "Scan complete".to_string()
                        } else {
                            "Clean complete".to_string()
                        };
                        self.status_toast = 120;
                        if self.scanning {
                            self.scanning = false;
                            self.last_scan_summary = summary;
                        }
                        self.cleaning = false;
                        self.progress = 0.0;
                        if was_cleaning && !self.settings.dry_run {
                            self.prune_after_clean();
                        }
                        still_active = false;
                    }
                    workers::WorkerMessage::Error(e) => {
                        self.add_log(&format!("ERROR: {}", e));
                        self.status = "Error".to_string();
                        self.status_toast = 120;
                        self.scanning = false;
                        self.cleaning = false;
                        still_active = false;
                    }
                    workers::WorkerMessage::Cancelled => {
                        self.add_log("Operation cancelled by user.");
                        self.status = "Cancelled".to_string();
                        self.status_toast = 60;
                        self.scanning = false;
                        self.cleaning = false;
                        still_active = false;
                    }
                }
            }
            if still_active {
                self.rx = Some(rx);
                ctx.request_repaint();
            } else {
                self.rx = None;
            }
        }
    }

    pub fn refresh_storage(&mut self) {
        self.storage_disks.clear();
        let disks = sysinfo::Disks::new_with_refreshed_list();
        for d in disks.list() {
            self.storage_disks.push(DiskEntry {
                name: d.name().to_string_lossy().into_owned(),
                mount: d.mount_point().display().to_string(),
                total: d.total_space(),
                available: d.available_space(),
            });
        }
        self.storage_loaded = true;
    }
}

// -----------------------------------------------------------------------------
// tab drawing
// -----------------------------------------------------------------------------

impl CleanerApp {
    fn draw_dashboard(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_source("dash_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| self.draw_dashboard_inner(ui));
    }

    fn draw_dashboard_inner(&mut self, ui: &mut egui::Ui) {
        let gap = 8.0;
        let w = ((ui.available_width() - 2.0 * gap) / 3.0).max(80.0);
        ui.horizontal(|ui| {
            ui.allocate_ui(egui::vec2(w, 64.0), |ui| {
                stat_card(
                    ui,
                    "FILES CLEANED",
                    format!("{}", self.total_files_cleaned),
                    "this session",
                );
            });
            ui.add_space(gap);
            ui.allocate_ui(egui::vec2(w, 64.0), |ui| {
                stat_card(
                    ui,
                    "SPACE FREED",
                    helpers::human_size(self.total_space_freed),
                    "this session",
                );
            });
            ui.add_space(gap);
            ui.allocate_ui(egui::vec2(w, 64.0), |ui| {
                stat_card(ui, "LAST SCAN", self.last_scan_summary.clone(), "");
            });
        });

        ui.add_space(10.0);

        card(ui, "Quick actions", |ui| {
            let busy = self.busy();
            ui.horizontal_wrapped(|ui| {
                if primary_button(ui, !busy, "Scan custom folder").clicked() {
                    self.tab = Tab::CustomClean;
                    self.start_custom_scan();
                }
                if primary_button(ui, !busy, "Scan system junk").clicked() {
                    self.tab = Tab::SystemCleaner;
                    self.start_system_scan();
                }
                if primary_button(ui, !busy, "Find duplicates").clicked() {
                    self.tab = Tab::Duplicates;
                    self.start_duplicates_scan();
                }
                if primary_button(ui, !busy, "Find large files").clicked() {
                    self.tab = Tab::LargeFiles;
                    self.start_large_files_scan();
                }
            });
        });

        ui.add_space(10.0);

        card(ui, "Safety", |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .checkbox(&mut self.settings.dry_run, "Dry run (preview only)")
                    .changed()
                {
                    self.settings.save();
                }
                if ui
                    .checkbox(&mut self.settings.use_trash, "Move to recycle bin")
                    .changed()
                {
                    self.settings.save();
                }
                if ui
                    .checkbox(&mut self.settings.secure_delete, "Secure delete (3-pass)")
                    .changed()
                {
                    self.settings.save();
                }
                if ui
                    .checkbox(&mut self.settings.confirm_clean, "Confirm before cleaning")
                    .changed()
                {
                    self.settings.save();
                }
            });
            if self.settings.dry_run {
                ui.label(
                    egui::RichText::new(
                        "Dry run is on — scans preview matches but nothing is deleted.",
                    )
                    .weak()
                    .small(),
                );
            } else {
                ui.colored_label(
                    warn_color(),
                    "Dry run is OFF — cleaning will delete files for real.",
                );
            }
        });

        ui.add_space(10.0);

        card(ui, "Recent activity", |ui| {
            if self.log.is_empty() {
                empty_state(ui, "No activity yet — start a scan to see it here.");
            } else {
                let start = self.log.len().saturating_sub(14);
                egui::ScrollArea::vertical()
                    .id_source("dash_log")
                    .max_height(190.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for line in &self.log[start..] {
                            ui.label(egui::RichText::new(line).monospace().small());
                        }
                    });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.small_button("Open log file").clicked() {
                        reveal_in_explorer(&self.settings.log_path());
                    }
                    if ui.small_button("Clear").clicked() {
                        self.log.clear();
                    }
                });
            }
        });
    }

    fn draw_custom_clean(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy();

        card(ui, "Scan setup", |ui| {
            dir_picker(ui, &mut self.custom.dir_path);
            ui.add_space(6.0);
            egui::Grid::new("custom_filters")
                .num_columns(2)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Extensions");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.custom.extensions)
                            .hint_text("tmp, log"),
                    );
                    ui.end_row();

                    ui.label("Glob pattern");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.custom.pattern)
                            .hint_text("*.tmp"),
                    );
                    ui.end_row();

                    ui.label("Older than (days)");
                    ui.add(
                        egui::Slider::new(&mut self.custom.older_than_days, 0..=36500)
                            .text("days"),
                    );
                    ui.end_row();

                    ui.label("Min size (bytes)");
                    ui.add(
                        egui::DragValue::new(&mut self.custom.min_size_bytes).speed(1000),
                    );
                    ui.end_row();

                    ui.label("Max size (bytes, 0 = no limit)");
                    ui.add(
                        egui::DragValue::new(&mut self.custom.max_size_bytes).speed(1000),
                    );
                    ui.end_row();

                    ui.label("Exclude directories");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.custom.exclude_dirs)
                            .hint_text("comma separated, absolute paths"),
                    );
                    ui.end_row();
                });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if primary_button(ui, !busy, "Scan").clicked() {
                    self.start_custom_scan();
                }
                let n = self.custom.matched_files.len();
                if danger_button(ui, !busy && n > 0, format!("Clean {} files", n))
                    .clicked()
                {
                    if self.settings.confirm_clean {
                        self.confirm_title = "Clean matched files?".to_string();
                        self.confirm_body = format!(
                            "Delete the {} matched files ({})?",
                            n,
                            helpers::human_size(self.custom.total_matched_size)
                        );
                        self.confirm_action = Some(ConfirmAction::CleanFiles);
                    } else {
                        self.start_clean_selected();
                    }
                }
                if ui
                    .add_enabled(n > 0, egui::Button::new("Export report"))
                    .clicked()
                {
                    match helpers::export_report(&self.custom.matched_files, "custom") {
                        Ok(p) => {
                            self.status = format!("Report saved: {}", p.display());
                            self.status_toast = 60;
                        }
                        Err(e) => {
                            self.status = format!("Export error: {}", e);
                            self.status_toast = 60;
                        }
                    }
                }
            });
        });

        ui.add_space(10.0);

        card(ui, "Results", |ui| {
            ui.label(format!(
                "Matched {} files · {}",
                self.custom.matched_files.len(),
                helpers::human_size(self.custom.total_matched_size)
            ));
            ui.add_space(4.0);
            matched_file_rows(ui, &self.custom.matched_files, "custom_scroll");
        });
    }

    fn draw_duplicates(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy();

        card(ui, "Scan setup", |ui| {
            dir_picker(ui, &mut self.duplicates.dir_path);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if primary_button(ui, !busy, "Scan for duplicates").clicked() {
                    self.start_duplicates_scan();
                }
                if ui
                    .add_enabled(
                        !busy && !self.duplicates.groups.is_empty(),
                        egui::Button::new("Select all"),
                    )
                    .clicked()
                {
                    self.duplicates.selected_files = self
                        .duplicates
                        .groups
                        .iter()
                        .flat_map(|g| g.files.iter().skip(1).cloned())
                        .collect();
                    self.duplicates.total_wasted = self.dup_wasted();
                }
                if ui
                    .add_enabled(
                        !busy && !self.duplicates.selected_files.is_empty(),
                        egui::Button::new("Clear selection"),
                    )
                    .clicked()
                {
                    self.duplicates.selected_files.clear();
                    self.duplicates.total_wasted = 0;
                }
                let n = self.duplicates.selected_files.len();
                if danger_button(ui, !busy && n > 0, format!("Clean {} selected", n))
                    .clicked()
                {
                    if self.settings.confirm_clean {
                        self.confirm_title = "Clean duplicates?".to_string();
                        self.confirm_body = format!(
                            "Delete {} selected duplicate files ({})? The first file in each group is kept.",
                            n,
                            helpers::human_size(self.duplicates.total_wasted)
                        );
                        self.confirm_action = Some(ConfirmAction::CleanDuplicates);
                    } else {
                        self.start_clean_duplicates();
                    }
                }
            });
            ui.label(
                egui::RichText::new(
                    "Two-stage hashing: groups by size, quick-hashes 8 KB, then full SHA-256.",
                )
                .weak()
                .small(),
            );
        });

        ui.add_space(10.0);

        card(ui, "Results", |ui| {
            ui.label(format!(
                "{} groups · {} reclaimable",
                self.duplicates.groups.len(),
                helpers::human_size(self.duplicates.total_wasted)
            ));
            ui.add_space(4.0);

            let groups = &self.duplicates.groups;
            let selected = &mut self.duplicates.selected_files;
            let mut changed = false;

            egui::ScrollArea::vertical()
                .id_source("duplicates_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if groups.is_empty() {
                        empty_state(ui, "No duplicate groups found yet — run a scan.");
                        return;
                    }
                    for group in groups {
                        ui.collapsing(
                            format!(
                                "{} files · {} each · {}",
                                group.files.len(),
                                helpers::human_size(group.size),
                                &group.hash[..8.min(group.hash.len())]
                            ),
                            |ui| {
                                for (i, file) in group.files.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        if i == 0 {
                                            ui.colored_label(accent(ui), "keep");
                                        } else {
                                            let mut on = selected.contains(file);
                                            if ui.checkbox(&mut on, "").changed() {
                                                changed = true;
                                                if on {
                                                    selected.push(file.clone());
                                                } else {
                                                    selected.retain(|x| x != file);
                                                }
                                            }
                                        }
                                        ui.monospace(file.display().to_string());
                                    });
                                }
                            },
                        );
                    }
                });

            if changed {
                self.duplicates.total_wasted = self.dup_wasted();
            }
        });
    }

    fn draw_large_files(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy();

        card(ui, "Scan setup", |ui| {
            dir_picker(ui, &mut self.large_files.dir_path);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Larger than");
                ui.add(
                    egui::Slider::new(&mut self.large_files.threshold_mb, 1..=10240)
                        .text("MB"),
                );
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if primary_button(ui, !busy, "Scan for large files").clicked() {
                    self.start_large_files_scan();
                }
                if ui
                    .add_enabled(
                        !busy && !self.large_files.files.is_empty(),
                        egui::Button::new("Select all"),
                    )
                    .clicked()
                {
                    self.large_files.selected = self
                        .large_files
                        .files
                        .iter()
                        .map(|f| f.path.clone())
                        .collect();
                }
                if ui
                    .add_enabled(
                        !busy && !self.large_files.selected.is_empty(),
                        egui::Button::new("Clear selection"),
                    )
                    .clicked()
                {
                    self.large_files.selected.clear();
                }
                let n = self.large_files.selected.len();
                if danger_button(ui, !busy && n > 0, format!("Clean {} selected", n))
                    .clicked()
                {
                    if self.settings.confirm_clean {
                        self.confirm_title = "Delete selected large files?".to_string();
                        self.confirm_body =
                            format!("Permanently remove the {} selected files?", n);
                        self.confirm_action = Some(ConfirmAction::CleanFiles);
                    } else {
                        self.start_clean_selected();
                    }
                }
                if ui
                    .add_enabled(
                        !self.large_files.files.is_empty(),
                        egui::Button::new("Export report"),
                    )
                    .clicked()
                {
                    let files: Vec<MatchedFile> = self
                        .large_files
                        .files
                        .iter()
                        .map(|f| MatchedFile {
                            path: f.path.clone(),
                            size: f.size,
                        })
                        .collect();
                    match helpers::export_report(&files, "large_files") {
                        Ok(p) => {
                            self.status = format!("Report saved: {}", p.display());
                            self.status_toast = 60;
                        }
                        Err(e) => {
                            self.status = format!("Export error: {}", e);
                            self.status_toast = 60;
                        }
                    }
                }
            });
        });

        ui.add_space(10.0);

        card(ui, "Results", |ui| {
            ui.label(format!(
                "{} files · {} total · {} selected",
                self.large_files.files.len(),
                helpers::human_size(self.large_files.total_size),
                self.large_files.selected.len()
            ));
            ui.add_space(4.0);

            let files = &self.large_files.files;
            let selected = &mut self.large_files.selected;
            egui::ScrollArea::vertical()
                .id_source("large_files_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if files.is_empty() {
                        empty_state(ui, "No large files found yet — run a scan.");
                        return;
                    }
                    for f in files {
                        ui.horizontal(|ui| {
                            let mut on = selected.contains(&f.path);
                            if ui.checkbox(&mut on, "").changed() {
                                if on {
                                    selected.push(f.path.clone());
                                } else {
                                    selected.retain(|x| x != &f.path);
                                }
                            }
                            ui.monospace(f.path.display().to_string());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(helpers::human_size(f.size))
                                            .weak(),
                                    );
                                    if ui.small_button("Locate").clicked() {
                                        if let Some(parent) = f.path.parent() {
                                            reveal_in_explorer(parent);
                                        }
                                    }
                                },
                            );
                        });
                    }
                });
        });
    }

    fn draw_system_cleaner(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy();

        card(ui, "Cleaning targets", |ui| {
            for target in &mut self.system.targets {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut target.enabled, "");
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&target.name).strong());
                        ui.label(
                            egui::RichText::new(&target.description).weak().small(),
                        );
                        ui.label(
                            egui::RichText::new(target.path.display().to_string())
                                .monospace()
                                .small()
                                .weak(),
                        );
                    });
                });
                ui.separator();
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if primary_button(ui, !busy, "Scan system").clicked() {
                    self.start_system_scan();
                }
                let n = self.system.matched_files.len();
                if danger_button(ui, !busy && n > 0, format!("Clean {} files", n))
                    .clicked()
                {
                    if self.settings.confirm_clean {
                        self.confirm_title = "Clean system files?".to_string();
                        self.confirm_body = format!(
                            "Delete the {} matched system files ({})?",
                            n,
                            helpers::human_size(self.system.total_matched_size)
                        );
                        self.confirm_action = Some(ConfirmAction::CleanFiles);
                    } else {
                        self.start_clean_selected();
                    }
                }
            });
        });

        ui.add_space(10.0);

        card(ui, "Results", |ui| {
            ui.label(format!(
                "Matched {} files · {}",
                self.system.matched_files.len(),
                helpers::human_size(self.system.total_matched_size)
            ));
            ui.add_space(4.0);
            matched_file_rows(ui, &self.system.matched_files, "system_scroll");
        });
    }

    fn draw_empty_folders(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy();

        card(ui, "Scan setup", |ui| {
            dir_picker(ui, &mut self.empty_folders.dir_path);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if primary_button(ui, !busy, "Scan for empty folders").clicked() {
                    self.start_empty_folders_scan();
                }
                let n = self.empty_folders.folders.len();
                if danger_button(ui, !busy && n > 0, format!("Remove {} folders", n))
                    .clicked()
                {
                    if self.settings.confirm_clean {
                        self.confirm_title = "Remove empty folders?".to_string();
                        self.confirm_body = format!(
                            "Remove {} empty folders (including cascades)?",
                            n
                        );
                        self.confirm_action = Some(ConfirmAction::CleanEmptyFolders);
                    } else {
                        self.start_clean_empty_folders();
                    }
                }
            });
            ui.label(
                egui::RichText::new(
                    "Cascade-aware: folders that become empty after their children are removed are included.",
                )
                .weak()
                .small(),
            );
        });

        ui.add_space(10.0);

        card(ui, "Results", |ui| {
            ui.label(format!(
                "Found {} empty folders",
                self.empty_folders.folders.len()
            ));
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .id_source("empty_folders_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if self.empty_folders.folders.is_empty() {
                        empty_state(ui, "No empty folders found yet — run a scan.");
                        return;
                    }
                    for f in &self.empty_folders.folders {
                        ui.monospace(f.display().to_string());
                    }
                });
        });
    }

    fn draw_storage(&mut self, ui: &mut egui::Ui) {
        if !self.storage_loaded {
            self.refresh_storage();
        }
        egui::ScrollArea::vertical()
            .id_source("storage_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| self.draw_storage_inner(ui));
    }

    fn draw_storage_inner(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Drives")
                    .strong()
                    .size(15.0),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Refresh").clicked() {
                    self.refresh_storage();
                }
            });
        });
        ui.add_space(4.0);

        if self.storage_disks.is_empty() {
            card(ui, "", |ui| {
                empty_state(ui, "No drives detected.");
            });
            return;
        }

        for disk in &self.storage_disks {
            let used = disk.total.saturating_sub(disk.available);
            let percent = if disk.total > 0 {
                (used as f64 / disk.total as f64) * 100.0
            } else {
                0.0
            };
            let bar_color = if percent >= 90.0 {
                danger_color()
            } else if percent >= 70.0 {
                warn_color()
            } else {
                accent(ui)
            };

            card(ui, "", |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(if disk.name.is_empty() {
                            disk.mount.clone()
                        } else {
                            format!("{}  ({})", disk.name, disk.mount)
                        })
                        .strong(),
                    );
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} free of {}",
                                    helpers::human_size(disk.available),
                                    helpers::human_size(disk.total)
                                ))
                                .weak(),
                            );
                        },
                    );
                });
                ui.add(
                    egui::ProgressBar::new((percent / 100.0) as f32)
                        .fill(bar_color)
                        .show_percentage()
                        .text(format!("{:.0}% full", percent)),
                );
            });
            ui.add_space(8.0);
        }

        ui.label(
            egui::RichText::new("Read-only view — nothing here deletes anything.")
                .weak()
                .small(),
        );
    }

    fn draw_changelog(&mut self, ui: &mut egui::Ui) {
        card(ui, "", |ui| {
            egui::ScrollArea::vertical()
                .id_source("changelog_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(CHANGELOG).monospace().small());
                });
        });
    }

    fn draw_about(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_source("about_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| self.draw_about_inner(ui));
    }

    fn draw_about_inner(&mut self, ui: &mut egui::Ui) {
        card(ui, "", |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Cleaner").size(22.0).strong());
                    ui.label(
                        egui::RichText::new(format!(
                            "Version {}",
                            env!("CARGO_PKG_VERSION")
                        ))
                        .weak(),
                    );
                });
            });
            ui.add_space(4.0);
            ui.label("A modern, fast, and safe system cleaning utility written in Rust.");
        });

        ui.add_space(10.0);

        card(ui, "Features", |ui| {
            for line in [
                "Custom file cleaning with filters",
                "Duplicate file finder (two-stage hashing)",
                "Large file finder",
                "System junk cleaner (temp files, caches)",
                "Empty folder cleaner (cascade-aware)",
                "Secure Delete (3-pass shredder)",
                "5 themes with animated transitions",
                "Recycle-bin deletion and dry-run safety",
                "Storage overview and exportable reports",
            ] {
                ui.label(format!("• {}", line));
            }
        });

        ui.add_space(10.0);

        card(ui, "Links & files", |ui| {
            ui.horizontal(|ui| {
                ui.label("Repository");
                ui.text_edit_singleline(&mut self.settings.github_url);
                if ui.button("Open").clicked() {
                    let _ = open::that(&self.settings.github_url);
                }
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Check for updates").clicked() {
                    let _ = open::that("https://github.com/w0wzahh/cleaner/releases");
                }
                if ui.button("Reveal settings file").clicked() {
                    reveal_in_explorer(&settings::settings_file_path());
                }
                if ui.button("Open history log").clicked() {
                    reveal_in_explorer(&self.settings.log_path());
                }
            });
        });

        ui.add_space(10.0);
        ui.label(egui::RichText::new("Written in Rust using egui.").weak().small());
    }
}

// -----------------------------------------------------------------------------
// eframe App
// -----------------------------------------------------------------------------

impl eframe::App for CleanerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.initial_theme_applied {
            themes::apply_spacing(ctx);
            ctx.set_visuals(self.settings.theme.visuals());
            self.theme_anim.to = self.settings.theme.visuals();
            self.initial_theme_applied = true;
        }

        self.update_theme_animation(ctx);
        self.poll_messages(ctx);

        if self.status_toast > 0 {
            self.status_toast -= 1;
            ctx.request_repaint();
            if self.status_toast == 0 {
                self.status = if self.busy() {
                    "Working...".to_string()
                } else {
                    "Ready".to_string()
                };
            }
        }

        // -- sidebar ----------------------------------------------------------
        egui::SidePanel::left("nav")
            .resizable(false)
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Cleaner").size(20.0).strong());
                        ui.label(
                            egui::RichText::new(format!(
                                "v{}",
                                env!("CARGO_PKG_VERSION")
                            ))
                            .weak()
                            .small(),
                        );
                    });
                });
                ui.add_space(16.0);

                nav_section(ui, "OVERVIEW");
                if nav_item(ui, self.tab, Tab::Dashboard, "Dashboard") {
                    self.tab = Tab::Dashboard;
                }
                if nav_item(ui, self.tab, Tab::Storage, "Storage") {
                    self.tab = Tab::Storage;
                }

                ui.add_space(8.0);
                nav_section(ui, "CLEANING");
                if nav_item(ui, self.tab, Tab::CustomClean, "Custom Clean") {
                    self.tab = Tab::CustomClean;
                }
                if nav_item(ui, self.tab, Tab::Duplicates, "Duplicates") {
                    self.tab = Tab::Duplicates;
                }
                if nav_item(ui, self.tab, Tab::LargeFiles, "Large Files") {
                    self.tab = Tab::LargeFiles;
                }
                if nav_item(ui, self.tab, Tab::SystemCleaner, "System Cleaner") {
                    self.tab = Tab::SystemCleaner;
                }
                if nav_item(ui, self.tab, Tab::EmptyFolders, "Empty Folders") {
                    self.tab = Tab::EmptyFolders;
                }

                ui.add_space(8.0);
                nav_section(ui, "APP");
                if nav_item(ui, self.tab, Tab::Changelog, "Changelog") {
                    self.tab = Tab::Changelog;
                }
                if nav_item(ui, self.tab, Tab::About, "About") {
                    self.tab = Tab::About;
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        if self.settings.dry_run {
                            ui.colored_label(warn_color(), "● Dry run on");
                        } else {
                            ui.colored_label(
                                egui::Color32::from_rgb(0x4C, 0xAF, 0x50),
                                "● Live mode",
                            );
                        }
                    });
                });
            });

        // -- header -----------------------------------------------------------
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(self.tab.title()).size(18.0).strong(),
                    );
                    ui.label(
                        egui::RichText::new(self.tab.subtitle()).weak().small(),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    if ui.button("Save settings").clicked() {
                        self.settings.save();
                        self.add_log("Settings saved.");
                    }
                    ui.label("Theme:");
                    let mut new_theme: Option<themes::Theme> = None;
                    egui::ComboBox::from_id_source("theme_combo")
                        .selected_text(self.settings.theme.label())
                        .show_ui(ui, |ui| {
                            for t in themes::Theme::all() {
                                if ui
                                    .selectable_label(
                                        *t == self.settings.theme,
                                        t.label(),
                                    )
                                    .clicked()
                                {
                                    new_theme = Some(*t);
                                }
                            }
                        });
                    if let Some(t) = new_theme {
                        self.pending_theme_change = Some(t);
                    }
                });
            });
            ui.add_space(6.0);
        });

        // Apply pending theme change (outside of header borrow)
        if let Some(t) = self.pending_theme_change.take() {
            self.change_theme(t, ctx);
        }

        // -- status bar -------------------------------------------------------
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                if self.busy() {
                    ui.spinner();
                }
                ui.label(&self.status);
                if self.busy() {
                    ui.separator();
                    ui.add(
                        egui::ProgressBar::new(self.progress)
                            .desired_width(180.0)
                            .show_percentage(),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .weak()
                            .small(),
                    );
                    if self.busy() && ui.button("Cancel").clicked() {
                        self.cancel();
                    }
                });
            });
            ui.add_space(4.0);
        });

        // -- main content -----------------------------------------------------
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            match self.tab {
                Tab::Dashboard => self.draw_dashboard(ui),
                Tab::CustomClean => self.draw_custom_clean(ui),
                Tab::Duplicates => self.draw_duplicates(ui),
                Tab::LargeFiles => self.draw_large_files(ui),
                Tab::SystemCleaner => self.draw_system_cleaner(ui),
                Tab::EmptyFolders => self.draw_empty_folders(ui),
                Tab::Storage => self.draw_storage(ui),
                Tab::Changelog => self.draw_changelog(ui),
                Tab::About => self.draw_about(ui),
            }
        });

        // -- confirmation dialog ----------------------------------------------
        if self.confirm_action.is_some() {
            let mut close = false;
            let mut do_action = false;
            egui::Window::new(&self.confirm_title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label(&self.confirm_body);
                    if self.settings.dry_run {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(
                                "Dry run is enabled — nothing will actually be deleted.",
                            )
                            .weak()
                            .small(),
                        );
                    }
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("Yes, continue")
                                        .strong()
                                        .color(egui::Color32::WHITE),
                                )
                                .fill(danger_color()),
                            )
                            .clicked()
                        {
                            do_action = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.confirm_action = None;
            }
            if do_action {
                self.handle_confirm();
            }
        }
    }
}

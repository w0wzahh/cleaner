//! Application state, event loop wiring, and the egui UI for Cleaner.
//!
//! Layout: left sidebar navigation, a header bar with the current page title and
//! global controls, a bottom status bar, and card-based content per tab.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use chrono::{Local, TimeZone};
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

/// Hand-drawn filled icons — emoji fonts aren't reliable on every machine.
/// Silhouette style, consistent with the reference design's iconography.
#[derive(Clone, Copy)]
enum Glyph {
    House,
    Chart,
    Disk,
    Broom,
    Copy,
    File,
    Screen,
    Trash,
    List,
    Info,
    Clock,
    Eye,
    Check,
}

/// Paint a small filled icon centered at `c`. `h` is the half-size in points;
/// `bg` is the surface color used for cut-out details.
fn paint_glyph(
    p: &egui::Painter,
    c: egui::Pos2,
    h: f32,
    g: Glyph,
    col: egui::Color32,
    bg: egui::Color32,
) {
    let pt = |dx: f32, dy: f32| c + egui::vec2(dx * h, dy * h);
    let poly = |p: &egui::Painter, pts: &[(f32, f32)], col: egui::Color32| {
        p.add(egui::Shape::convex_polygon(
            pts.iter().map(|&(x, y)| pt(x, y)).collect(),
            col,
            egui::Stroke::NONE,
        ));
    };
    let srect =
        |x0: f32, y0: f32, x1: f32, y1: f32, r: f32, col: egui::Color32| {
            p.rect_filled(
                egui::Rect::from_min_max(pt(x0, y0), pt(x1, y1)),
                r,
                col,
            );
        };
    let scirc = |dx: f32, dy: f32, rad: f32, col: egui::Color32| {
        p.circle_filled(pt(dx, dy), rad * h, col);
    };
    match g {
        Glyph::House => {
            poly(
                p,
                &[
                    (-1.0, 0.05),
                    (0.0, -0.95),
                    (1.0, 0.05),
                    (0.72, 0.05),
                    (0.72, 0.9),
                    (-0.72, 0.9),
                    (-0.72, 0.05),
                ],
                col,
            );
        }
        Glyph::Chart => {
            srect(-0.9, 0.15, -0.5, 0.9, h * 0.1, col);
            srect(-0.2, -0.35, 0.2, 0.9, h * 0.1, col);
            srect(0.5, -0.85, 0.9, 0.9, h * 0.1, col);
        }
        Glyph::Disk => {
            srect(-1.0, -0.7, 1.0, 0.7, h * 0.25, col);
            scirc(0.55, 0.0, 0.22, bg);
            srect(-0.75, -0.12, -0.3, 0.12, h * 0.1, bg);
        }
        Glyph::Broom => {
            poly(
                p,
                &[(0.38, -0.95), (0.62, -0.78), (-0.05, -0.05), (-0.28, -0.22)],
                col,
            );
            poly(
                p,
                &[
                    (-0.35, -0.15),
                    (-0.95, 0.5),
                    (-0.45, 0.95),
                    (0.1, 0.8),
                    (0.0, -0.05),
                ],
                col,
            );
            // Ferrule band, cut out of the silhouette.
            p.line_segment(
                [pt(-0.42, -0.18), pt(-0.08, 0.08)],
                egui::Stroke::new(h * 0.16, bg),
            );
        }
        Glyph::Copy => {
            srect(-0.2, -0.9, 0.9, 0.35, h * 0.18, col.gamma_multiply(0.45));
            srect(-0.9, -0.25, 0.35, 0.9, h * 0.18, col);
        }
        Glyph::File => {
            poly(
                p,
                &[
                    (-0.7, -0.9),
                    (0.2, -0.9),
                    (0.7, -0.4),
                    (0.7, 0.9),
                    (-0.7, 0.9),
                ],
                col,
            );
            poly(
                p,
                &[(0.2, -0.9), (0.7, -0.4), (0.2, -0.4)],
                bg,
            );
        }
        Glyph::Screen => {
            srect(-1.0, -0.75, 1.0, 0.3, h * 0.15, col);
            srect(-0.15, 0.3, 0.15, 0.62, h * 0.05, col);
            srect(-0.5, 0.62, 0.5, 0.82, h * 0.08, col);
        }
        Glyph::Trash => {
            srect(-0.85, -0.72, 0.85, -0.5, h * 0.1, col);
            srect(-0.3, -0.95, 0.3, -0.72, h * 0.08, col);
            poly(
                p,
                &[(-0.62, -0.35), (0.62, -0.35), (0.42, 0.9), (-0.42, 0.9)],
                col,
            );
        }
        Glyph::List => {
            for i in -1..=1 {
                let y = i as f32 * 0.55;
                scirc(-0.7, y, 0.14, col);
                srect(-0.3, y - 0.11, 0.9, y + 0.11, h * 0.1, col);
            }
        }
        Glyph::Info => {
            scirc(0.0, 0.0, 0.95, col);
            scirc(0.0, -0.42, 0.16, bg);
            srect(-0.13, -0.08, 0.13, 0.6, h * 0.1, bg);
        }
        Glyph::Clock => {
            scirc(0.0, 0.0, 0.95, col);
            p.line_segment(
                [c, pt(0.0, -0.5)],
                egui::Stroke::new(h * 0.22, bg),
            );
            p.line_segment(
                [c, pt(0.38, 0.2)],
                egui::Stroke::new(h * 0.22, bg),
            );
        }
        Glyph::Eye => {
            poly(
                p,
                &[
                    (-1.0, 0.0),
                    (-0.45, -0.6),
                    (0.45, -0.6),
                    (1.0, 0.0),
                    (0.45, 0.6),
                    (-0.45, 0.6),
                ],
                col,
            );
            scirc(0.0, 0.0, 0.26, bg);
        }
        Glyph::Check => {
            scirc(0.0, 0.0, 0.95, col);
            p.add(egui::Shape::line(
                vec![pt(-0.45, 0.05), pt(-0.15, 0.35), pt(0.5, -0.3)],
                egui::Stroke::new(h * 0.24, bg),
            ));
        }
    }
}

/// Sidebar navigation entry; returns true when clicked.
fn nav_item(ui: &mut egui::Ui, current: Tab, target: Tab, g: Glyph, label: &str) -> bool {
    let w = ui.available_width();
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(w, 30.0), egui::Sense::click());
    let selected = current == target;
    let hovered = resp.hovered();
    if ui.is_rect_visible(rect) {
        let acc = accent(ui);
        let p = ui.painter().with_clip_rect(rect);
        let pill = if selected {
            Some(acc.gamma_multiply(0.18))
        } else if hovered {
            Some(ui.visuals().faint_bg_color.gamma_multiply(1.4))
        } else {
            None
        };
        if let Some(fill) = pill {
            p.rect_filled(rect.shrink(1.0), 7.0, fill);
        }
        let bg = pill.unwrap_or(ui.visuals().panel_fill);
        let col = if selected {
            acc
        } else if hovered {
            ui.visuals().text_color()
        } else {
            ui.visuals().text_color().gamma_multiply(0.72)
        };
        paint_glyph(
            &p,
            egui::pos2(rect.min.x + 16.0, rect.center().y),
            7.5,
            g,
            col,
            bg,
        );
        p.text(
            egui::pos2(rect.min.x + 32.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(13.0),
            col,
        );
    }
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
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

/// Big glowing ring used as the dashboard's main action. `progress == None`
/// shows an idle "START" ring; `Some(p)` turns it into a progress arc, and
/// `Some(~0)` becomes a spinning indeterminate arc.
fn start_ring(
    ui: &mut egui::Ui,
    progress: Option<f32>,
    accent2: egui::Color32,
) -> egui::Response {
    let accent = accent(ui);
    let size = 170.0;
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter().with_clip_rect(rect);
        let c = rect.center();
        let r = size / 2.0 - 16.0;
        let boost = if resp.hovered() { 1.25 } else { 1.0 };

        // Soft outer glow.
        for i in 1..=4u32 {
            p.circle_stroke(
                c,
                r + i as f32 * 4.5,
                egui::Stroke::new(2.0_f32, accent.gamma_multiply(0.10 / i as f32)),
            );
        }
        // Dim track.
        p.circle_stroke(c, r, egui::Stroke::new(7.0_f32, accent.gamma_multiply(0.15)));

        let now = ui.ctx().input(|i| i.time) as f32;
        let (start_deg, sweep_deg, label, label_size) = match progress {
            None => (-90.0, 360.0, "START".to_string(), 19.0),
            Some(pr) if pr <= 0.001 => {
                ((now * 220.0) % 360.0 - 90.0, 100.0, "…".to_string(), 24.0)
            }
            Some(pr) => (
                -90.0,
                pr.clamp(0.0, 1.0) * 360.0,
                format!("{:.0}%", pr.clamp(0.0, 1.0) * 100.0),
                24.0,
            ),
        };

        // Gradient arc in ~5-degree segments.
        let segs = 72usize;
        for i in 0..segs {
            let t0 = i as f32 / segs as f32;
            let t1 = (i + 1) as f32 / segs as f32;
            if sweep_deg * t0 >= sweep_deg {
                break;
            }
            let a0 = (start_deg + sweep_deg * t0).to_radians();
            let a1 = (start_deg + sweep_deg * t1).to_radians();
            let col = crate::themes::lerp_color(accent2, accent, t0)
                .gamma_multiply(boost);
            let p0 = c + egui::vec2(a0.cos() * r, a0.sin() * r);
            let p1 = c + egui::vec2(a1.cos() * r, a1.sin() * r);
            p.line_segment([p0, p1], egui::Stroke::new(7.0_f32, col));
        }

        p.text(
            c,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(label_size),
            egui::Color32::WHITE,
        );
    }
    if resp.hovered() && progress.is_none() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Clickable dashboard tile: icon chip + title + description.
fn tool_tile(
    ui: &mut egui::Ui,
    width: f32,
    icon: Glyph,
    title: &str,
    desc: &str,
) -> egui::Response {
    let h = 76.0;
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(width, h), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let accent = accent(ui);
        let p = ui.painter().with_clip_rect(rect);
        let hovered = resp.hovered();
        let fill = if hovered {
            ui.visuals().faint_bg_color.gamma_multiply(1.45)
        } else {
            ui.visuals().faint_bg_color
        };
        let border = if hovered {
            accent.gamma_multiply(0.7)
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke.color
        };
        p.rect_filled(rect, 10.0, fill);
        p.rect_stroke(rect, 10.0, egui::Stroke::new(1.0_f32, border));

        let icon_c = rect.min + egui::vec2(32.0, 30.0);
        p.circle_filled(icon_c, 15.0, accent.gamma_multiply(0.14));
        paint_glyph(&p, icon_c, 8.0, icon, accent, fill);
        p.text(
            rect.min + egui::vec2(58.0, 18.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::proportional(14.5),
            ui.visuals().text_color(),
        );
        p.text(
            rect.min + egui::vec2(58.0, 42.0),
            egui::Align2::LEFT_TOP,
            desc,
            egui::FontId::proportional(11.0),
            ui.visuals().weak_text_color(),
        );
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Human label for a schedule interval in hours.
fn schedule_interval_label(hours: u32) -> &'static str {
    match hours {
        1 => "1 hour",
        6 => "6 hours",
        12 => "12 hours",
        24 => "24 hours (daily)",
        168 => "7 days (weekly)",
        _ => "24 hours (daily)",
    }
}

/// Little bar chart: files cleaned per day over the last week.
fn week_chart(ui: &mut egui::Ui, history: &BTreeMap<String, u64>) {
    let today = Local::now().date_naive();
    let days: Vec<(String, u64)> = (0..7)
        .rev()
        .map(|i| {
            let d = today - chrono::TimeDelta::days(i);
            (
                d.format("%a").to_string(),
                *history
                    .get(&d.format("%Y-%m-%d").to_string())
                    .unwrap_or(&0),
            )
        })
        .collect();
    let real_max = days.iter().map(|d| d.1).max().unwrap_or(0);
    let max = real_max.max(1) as f32;
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 116.0), egui::Sense::hover());
    let p = ui.painter().with_clip_rect(rect);
    let accent = accent(ui);
    let weak = ui.visuals().weak_text_color();
    let n = days.len() as f32;
    let gap = 10.0;
    let bw = (rect.width() - gap * (n - 1.0)).max(4.0) / n;
    let chart_h = rect.height() - 22.0;

    if real_max == 0 {
        p.text(
            rect.center() - egui::vec2(0.0, 8.0),
            egui::Align2::CENTER_CENTER,
            "No cleans recorded yet this week",
            egui::FontId::proportional(12.0),
            weak,
        );
    }

    for (i, (day, count)) in days.iter().enumerate() {
        let x0 = rect.min.x + i as f32 * (bw + gap);
        let bh = (*count as f32 / max) * (chart_h - 16.0);
        let bar = egui::Rect::from_min_size(
            egui::pos2(x0, rect.min.y + chart_h - bh),
            egui::vec2(bw, bh.max(2.0)),
        );
        let col = if i == days.len() - 1 {
            accent
        } else {
            accent.gamma_multiply(0.4)
        };
        p.rect_filled(bar, 4.0, col);
        if *count > 0 {
            p.text(
                egui::pos2(x0 + bw / 2.0, bar.min.y - 8.0),
                egui::Align2::CENTER_CENTER,
                format!("{}", count),
                egui::FontId::proportional(10.0),
                weak,
            );
        }
        p.text(
            egui::pos2(x0 + bw / 2.0, rect.max.y - 9.0),
            egui::Align2::CENTER_CENTER,
            day,
            egui::FontId::proportional(10.0),
            weak,
        );
    }
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
    /// Set when a scheduled scan should auto-clean once its results arrive.
    pub pending_auto_clean: bool,
    pub initial_theme_applied: bool,

    pub custom: CustomCleanerState,
    pub duplicates: DuplicateState,
    pub large_files: LargeFilesState,
    pub system: SystemCleanerState,
    pub empty_folders: EmptyFoldersState,
    pub folder_sizes: FolderSizesState,

    pub storage_disks: Vec<DiskEntry>,
    pub storage_loaded: bool,

    pub new_target_name: String,
    pub new_target_path: String,

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
            pending_auto_clean: false,
            custom: CustomCleanerState::default(),
            duplicates: DuplicateState::default(),
            large_files: LargeFilesState::default(),
            system: SystemCleanerState::default(),
            empty_folders: EmptyFoldersState::default(),
            folder_sizes: FolderSizesState::default(),
            storage_disks: Vec::new(),
            storage_loaded: false,
            new_target_name: String::new(),
            new_target_path: String::new(),
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
            app.empty_folders.dir_path = d.clone();
            app.folder_sizes.dir_path = d;
        }
        // Restore lifetime counters and user-defined clean targets.
        app.total_files_cleaned = app.settings.total_files_cleaned;
        app.total_space_freed = app.settings.total_space_freed;
        app.rebuild_custom_targets();
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

    /// Sync user-defined system-clean targets from settings into the UI list.
    fn rebuild_custom_targets(&mut self) {
        self.system.targets.retain(|t| !t.custom);
        for ct in &self.settings.custom_targets {
            self.system.targets.push(SystemCleanTarget {
                name: ct.name.clone(),
                path: PathBuf::from(&ct.path),
                description: "Custom target".to_string(),
                enabled: true,
                custom: true,
            });
        }
    }

    fn add_custom_target(&mut self) {
        let name = self.new_target_name.trim().to_string();
        let path = self.new_target_path.trim().to_string();
        if name.is_empty() || path.is_empty() {
            return;
        }
        self.settings
            .custom_targets
            .push(CustomTarget { name: name.clone(), path });
        self.rebuild_custom_targets();
        self.settings.save();
        self.add_log(&format!("Added custom target: {}", name));
        self.new_target_name.clear();
        self.new_target_path.clear();
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

    /// Fire a scheduled scan if one is due. Runs at most once per interval
    /// and only while the app is open.
    fn maybe_run_scheduled(&mut self) {
        if !self.settings.schedule_enabled || self.busy() {
            return;
        }
        let interval = (self.settings.schedule_hours.max(1) as i64) * 3600;
        let now = Local::now().timestamp();
        if self.settings.schedule_last_run + interval > now {
            return;
        }
        self.settings.schedule_last_run = now;
        self.settings.save();
        self.add_log("Scheduled scan triggered.");
        self.pending_auto_clean = self.settings.schedule_auto_clean;
        match self.settings.schedule_target {
            crate::settings::ScheduleTarget::System => {
                self.tab = Tab::SystemCleaner;
                self.start_system_scan();
            }
            crate::settings::ScheduleTarget::Custom => {
                self.tab = Tab::CustomClean;
                self.start_custom_scan();
            }
        }
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
        self.custom.selected.clear();
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
        let protected = self.settings.protected_list();
        let rec = self.settings.recursive;
        let hidden = self.settings.include_hidden;
        self.add_log("Starting custom scan...");
        thread::spawn(move || {
            workers::custom_scan_worker(
                dir, ext, days, min, max, pat, excl, protected, rec, hidden, cancel, tx,
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

    pub fn start_folder_sizes_scan(&mut self) {
        if self.busy() {
            return;
        }
        self.settings.save();
        self.scanning = true;
        self.progress = 0.0;
        self.folder_sizes.entries.clear();
        self.folder_sizes.total = 0;
        self.log.clear();
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let dir = self.folder_sizes.dir_path.clone();
        self.add_log("Starting folder size analysis...");
        thread::spawn(move || workers::folder_sizes_worker(dir, cancel, tx));
        self.rx = Some(rx);
        self.status = "Analyzing folder sizes...".to_string();
        self.status_toast = 0;
    }

    pub fn start_clean_selected(&mut self) {
        if self.busy() {
            return;
        }
        let files: Vec<MatchedFile> = match self.tab {
            Tab::CustomClean => self
                .custom
                .matched_files
                .iter()
                .filter(|f| self.custom.selected.contains(&f.path))
                .cloned()
                .collect(),
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
        let protected = self.settings.protected_list();
        self.add_log("Starting cleaning...");
        thread::spawn(move || {
            workers::clean_files(files, use_trash, dry_run, secure_delete, protected, cancel, tx)
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
        let protected = self.settings.protected_list();
        let matched: Vec<MatchedFile> = files
            .into_iter()
            .map(|p| {
                let size = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                MatchedFile { path: p, size }
            })
            .collect();
        self.add_log("Starting duplicate cleanup...");
        thread::spawn(move || {
            workers::clean_files(matched, use_trash, dry_run, secure_delete, protected, cancel, tx)
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
        let protected = self.settings.protected_list();
        self.add_log("Removing empty folders...");
        thread::spawn(move || {
            workers::clean_folders(folders, dry_run, use_trash, protected, cancel, tx)
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
                let sel = std::mem::take(&mut self.custom.selected);
                self.custom
                    .matched_files
                    .retain(|f| !sel.contains(&f.path));
                self.custom.total_matched_size =
                    self.custom.matched_files.iter().map(|f| f.size).sum();
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
                        self.custom.selected =
                            files.iter().map(|f| f.path.clone()).collect();
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
                    workers::WorkerMessage::FolderSizes(entries) => {
                        self.folder_sizes.total =
                            entries.iter().map(|e| e.size).sum();
                        self.folder_sizes.entries = entries;
                    }
                    workers::WorkerMessage::CleanStats { deleted, freed } => {
                        self.total_files_cleaned += deleted;
                        self.total_space_freed += freed;
                        self.settings.total_files_cleaned += deleted;
                        self.settings.total_space_freed += freed;
                        let today = Local::now().format("%Y-%m-%d").to_string();
                        *self.settings.clean_history.entry(today).or_default() +=
                            deleted;
                        self.settings.last_clean =
                            Local::now().format("%Y-%m-%d %H:%M").to_string();
                        while self.settings.clean_history.len() > 60 {
                            if let Some(k) =
                                self.settings.clean_history.keys().next().cloned()
                            {
                                self.settings.clean_history.remove(&k);
                            }
                        }
                        self.settings.save();
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
                            if self.pending_auto_clean {
                                self.pending_auto_clean = false;
                                if self.settings.dry_run {
                                    self.add_log(
                                        "Scheduled auto-clean skipped — dry run is on.",
                                    );
                                } else {
                                    self.add_log("Scheduled auto-clean starting...");
                                    self.start_clean_selected();
                                }
                            }
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
        // Hero: welcome text + the big glowing START ring.
        card(ui, "", |ui| {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    ui.add_space(26.0);
                    ui.label(egui::RichText::new("Welcome to").size(24.0));
                    ui.label(
                        egui::RichText::new("Cleaner")
                            .size(40.0)
                            .strong()
                            .color(accent(ui)),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Start a smart scan of your system.")
                            .weak()
                            .size(13.0),
                    );
                    ui.add_space(4.0);
                    if !self.settings.last_clean.is_empty() {
                        ui.label(
                            egui::RichText::new(format!(
                                "Last clean: {}",
                                self.settings.last_clean
                            ))
                            .weak()
                            .small(),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("No cleans yet.")
                                .weak()
                                .small(),
                        );
                    }
                });
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.add_space(10.0);
                        let busy = self.busy();
                        let resp = start_ring(
                            ui,
                            if busy { Some(self.progress) } else { None },
                            self.settings.theme.accent2(),
                        );
                        if resp.clicked() && !busy {
                            self.tab = Tab::SystemCleaner;
                            self.start_system_scan();
                        }
                    },
                );
            });
        });

        ui.add_space(10.0);

        // Stats row
        let gap = 8.0;
        let w = ((ui.available_width() - 2.0 * gap) / 3.0).max(80.0);
        ui.horizontal(|ui| {
            ui.allocate_ui(egui::vec2(w, 64.0), |ui| {
                stat_card(
                    ui,
                    "FILES CLEANED",
                    format!("{}", self.total_files_cleaned),
                    "all time",
                );
            });
            ui.add_space(gap);
            ui.allocate_ui(egui::vec2(w, 64.0), |ui| {
                stat_card(
                    ui,
                    "SPACE FREED",
                    helpers::human_size(self.total_space_freed),
                    "all time",
                );
            });
            ui.add_space(gap);
            ui.allocate_ui(egui::vec2(w, 64.0), |ui| {
                stat_card(ui, "LAST SCAN", self.last_scan_summary.clone(), "");
            });
        });

        ui.add_space(10.0);

        // Tool tiles — quick navigation, like the reference layout.
        card(ui, "Tools", |ui| {
            let tiles: [(Tab, Glyph, &str, &str); 6] = [
                (Tab::CustomClean, Glyph::Broom, "Custom Clean", "Filter and clean any folder"),
                (Tab::Duplicates, Glyph::Copy, "Duplicates", "Find and remove copies"),
                (Tab::LargeFiles, Glyph::File, "Large Files", "Locate the space hogs"),
                (Tab::SystemCleaner, Glyph::Screen, "System Cleaner", "Temp files and caches"),
                (Tab::EmptyFolders, Glyph::Trash, "Empty Folders", "Cascade-aware cleanup"),
                (Tab::FolderSizes, Glyph::Chart, "Folder Sizes", "See where space went"),
            ];
            let tw = ((ui.available_width() - 16.0) / 3.0).max(60.0);
            let mut go: Option<Tab> = None;
            for row in tiles.chunks(3) {
                ui.horizontal(|ui| {
                    for (i, (t, icon, title, desc)) in row.iter().enumerate() {
                        if tool_tile(ui, tw, *icon, title, desc).clicked() {
                            go = Some(*t);
                        }
                        if i + 1 < row.len() {
                            ui.add_space(8.0);
                        }
                    }
                });
                ui.add_space(8.0);
            }
            if let Some(t) = go {
                self.tab = t;
            }
        });

        ui.add_space(10.0);

        // Chart + quick actions side by side.
        let total_w = ui.available_width();
        let chart_w = (total_w * 0.56).max(240.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(chart_w, 10.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    card(ui, "Files cleaned — last 7 days", |ui| {
                        week_chart(ui, &self.settings.clean_history);
                    });
                },
            );
            ui.add_space(8.0);
            let busy = self.busy();
            ui.vertical(|ui| {
                card(ui, "Quick actions", |ui| {
                    if primary_button(ui, !busy, "Scan custom folder").clicked() {
                        self.tab = Tab::CustomClean;
                        self.start_custom_scan();
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
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(
                    "Protected paths — never scanned or deleted (one per line):",
                )
                .weak()
                .small(),
            );
            if ui
                .add(
                    egui::TextEdit::multiline(&mut self.settings.protected_paths)
                        .desired_width(f32::INFINITY)
                        .desired_rows(3)
                        .hint_text("C:\\Users\\you\\Documents\nD:\\KeepThese"),
                )
                .changed()
            {
                self.settings.save();
            }
        });

        ui.add_space(10.0);

        // Scheduled scans
        card(ui, "Scheduler", |ui| {
            ui.horizontal_wrapped(|ui| {
                let (r, _) =
                    ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                paint_glyph(
                    &ui.painter().with_clip_rect(r),
                    r.center(),
                    7.5,
                    Glyph::Clock,
                    accent(ui),
                    ui.visuals().faint_bg_color,
                );
                if ui
                    .checkbox(&mut self.settings.schedule_enabled, "Run on a schedule")
                    .changed()
                {
                    self.settings.save();
                }
                ui.label("every");
                let mut h = self.settings.schedule_hours;
                egui::ComboBox::from_id_source("sched_hours")
                    .selected_text(schedule_interval_label(h))
                    .show_ui(ui, |ui| {
                        for opt in [1u32, 6, 12, 24, 168] {
                            if ui
                                .selectable_label(h == opt, schedule_interval_label(opt))
                                .clicked()
                            {
                                h = opt;
                            }
                        }
                    });
                if h != self.settings.schedule_hours {
                    self.settings.schedule_hours = h;
                    self.settings.save();
                }
                ui.label("on");
                let mut t = self.settings.schedule_target;
                egui::ComboBox::from_id_source("sched_target")
                    .selected_text(t.label())
                    .show_ui(ui, |ui| {
                        for o in crate::settings::ScheduleTarget::all() {
                            if ui.selectable_label(t == *o, o.label()).clicked() {
                                t = *o;
                            }
                        }
                    });
                if t != self.settings.schedule_target {
                    self.settings.schedule_target = t;
                    self.settings.save();
                }
                if ui
                    .checkbox(
                        &mut self.settings.schedule_auto_clean,
                        "auto-clean after scan",
                    )
                    .changed()
                {
                    self.settings.save();
                }
            });
            if self.settings.schedule_enabled {
                let next = if self.settings.schedule_last_run == 0 {
                    "first run on the next check".to_string()
                } else {
                    let ts = self.settings.schedule_last_run
                        + self.settings.schedule_hours.max(1) as i64 * 3600;
                    match Local.timestamp_opt(ts, 0).single() {
                        Some(dt) => format!("next run {}", dt.format("%Y-%m-%d %H:%M")),
                        None => "next run soon".to_string(),
                    }
                };
                ui.label(
                    egui::RichText::new(format!(
                        "{} — runs while the app is open.",
                        next
                    ))
                    .weak()
                    .small(),
                );
            } else {
                ui.label(
                    egui::RichText::new(
                        "Off. When enabled, a scan runs automatically while the app is open.",
                    )
                    .weak()
                    .small(),
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

            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Presets:").weak().small());
                if ui.small_button("Temp & logs").clicked() {
                    self.custom.extensions = "tmp,log,bak,dmp".to_string();
                    self.custom.pattern.clear();
                    self.custom.older_than_days = 0;
                    self.custom.min_size_bytes = 0;
                    self.custom.max_size_bytes = 0;
                }
                if ui.small_button("Old files (30d+)").clicked() {
                    self.custom.older_than_days = 30;
                }
                if ui.small_button("Big media (50MB+)").clicked() {
                    self.custom.extensions =
                        "mp4,mkv,avi,mov,mp3,flac,wav".to_string();
                    self.custom.min_size_bytes = 50 * 1024 * 1024;
                    self.custom.max_size_bytes = 0;
                    self.custom.pattern.clear();
                }
                if ui.small_button("Images").clicked() {
                    self.custom.extensions =
                        "png,jpg,jpeg,gif,bmp,webp".to_string();
                }
                if ui.small_button("Old Downloads").clicked() {
                    if let Some(d) = dirs::download_dir() {
                        self.custom.dir_path = d.display().to_string();
                    }
                    self.custom.older_than_days = 30;
                    self.custom.extensions.clear();
                    self.custom.pattern.clear();
                }
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if primary_button(ui, !busy, "Scan").clicked() {
                    self.start_custom_scan();
                }
                if ui
                    .add_enabled(
                        !busy && !self.custom.matched_files.is_empty(),
                        egui::Button::new("Select all"),
                    )
                    .clicked()
                {
                    self.custom.selected = self
                        .custom
                        .matched_files
                        .iter()
                        .map(|f| f.path.clone())
                        .collect();
                }
                if ui
                    .add_enabled(
                        !busy && !self.custom.selected.is_empty(),
                        egui::Button::new("Clear"),
                    )
                    .clicked()
                {
                    self.custom.selected.clear();
                }
                let n = self.custom.selected.len();
                let sel_size: u64 = self
                    .custom
                    .matched_files
                    .iter()
                    .filter(|f| self.custom.selected.contains(&f.path))
                    .map(|f| f.size)
                    .sum();
                if danger_button(ui, !busy && n > 0, format!("Clean {} files", n))
                    .clicked()
                {
                    if self.settings.confirm_clean {
                        self.confirm_title = "Clean selected files?".to_string();
                        self.confirm_body = format!(
                            "Delete the {} selected files ({})?",
                            n,
                            helpers::human_size(sel_size)
                        );
                        self.confirm_action = Some(ConfirmAction::CleanFiles);
                    } else {
                        self.start_clean_selected();
                    }
                }
                if ui
                    .add_enabled(
                        !self.custom.matched_files.is_empty(),
                        egui::Button::new("Export report"),
                    )
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
                "Matched {} files · {} · {} selected",
                self.custom.matched_files.len(),
                helpers::human_size(self.custom.total_matched_size),
                self.custom.selected.len()
            ));

            // File-type breakdown: which extensions account for the size.
            if !self.custom.matched_files.is_empty() {
                let mut by_ext: BTreeMap<String, (u64, u64)> = BTreeMap::new();
                for f in &self.custom.matched_files {
                    let ext = f
                        .path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_lowercase())
                        .unwrap_or_else(|| "(no ext)".to_string());
                    let ent = by_ext.entry(ext).or_default();
                    ent.0 += 1;
                    ent.1 += f.size;
                }
                let mut ranked: Vec<(String, (u64, u64))> =
                    by_ext.into_iter().collect();
                ranked.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));
                ranked.truncate(10);
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    for (ext, (count, size)) in &ranked {
                        ui.label(
                            egui::RichText::new(format!(
                                ".{} {} · {}",
                                ext,
                                count,
                                helpers::human_size(*size)
                            ))
                            .small()
                            .weak(),
                        );
                    }
                });
            }
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Filter:").weak().small());
                ui.add(
                    egui::TextEdit::singleline(&mut self.custom.filter)
                        .hint_text("type to filter results")
                        .desired_width(220.0),
                );
                if !self.custom.filter.is_empty() && ui.small_button("Clear").clicked() {
                    self.custom.filter.clear();
                }
            });
            ui.add_space(4.0);

            let filter = self.custom.filter.to_lowercase();
            let files = &self.custom.matched_files;
            let selected = &mut self.custom.selected;
            egui::ScrollArea::vertical()
                .id_source("custom_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if files.is_empty() {
                        empty_state(ui, "Nothing here yet — run a scan.");
                        return;
                    }
                    for f in files.iter().filter(|f| {
                        filter.is_empty()
                            || f.path
                                .to_string_lossy()
                                .to_lowercase()
                                .contains(&filter)
                    }) {
                        ui.horizontal(|ui| {
                            let mut on = selected.contains(&f.path);
                            if ui.checkbox(&mut on, "").changed() {
                                if on {
                                    selected.insert(f.path.clone());
                                } else {
                                    selected.remove(&f.path);
                                }
                            }
                            ui.monospace(f.path.display().to_string());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(helpers::human_size(
                                            f.size,
                                        ))
                                        .weak(),
                                    );
                                },
                            );
                        });
                    }
                });
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

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Filter:").weak().small());
                ui.add(
                    egui::TextEdit::singleline(&mut self.duplicates.filter)
                        .hint_text("type to filter results")
                        .desired_width(220.0),
                );
                if !self.duplicates.filter.is_empty()
                    && ui.small_button("Clear").clicked()
                {
                    self.duplicates.filter.clear();
                }
            });
            ui.add_space(4.0);

            let filter = self.duplicates.filter.to_lowercase();
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
                    for group in groups.iter().filter(|g| {
                        filter.is_empty()
                            || g.files.iter().any(|f| {
                                f.to_string_lossy()
                                    .to_lowercase()
                                    .contains(&filter)
                            })
                    }) {
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

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Filter:").weak().small());
                ui.add(
                    egui::TextEdit::singleline(&mut self.large_files.filter)
                        .hint_text("type to filter results")
                        .desired_width(220.0),
                );
                if !self.large_files.filter.is_empty()
                    && ui.small_button("Clear").clicked()
                {
                    self.large_files.filter.clear();
                }
            });
            ui.add_space(4.0);

            let filter = self.large_files.filter.to_lowercase();
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
                    for f in files.iter().filter(|f| {
                        filter.is_empty()
                            || f.path
                                .to_string_lossy()
                                .to_lowercase()
                                .contains(&filter)
                    }) {
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
            let mut remove_idx: Option<usize> = None;
            for (i, target) in self.system.targets.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut target.enabled, "");
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&target.name).strong());
                            if target.custom {
                                ui.label(
                                    egui::RichText::new("custom").weak().small(),
                                );
                                if ui.small_button("✕").clicked() {
                                    remove_idx = Some(i);
                                }
                            }
                        });
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
            if let Some(i) = remove_idx {
                let removed = self.system.targets.remove(i);
                let path_str = removed.path.display().to_string();
                self.settings
                    .custom_targets
                    .retain(|c| c.path != path_str);
                self.settings.save();
                self.add_log(&format!("Removed custom target: {}", removed.name));
            }

            ui.collapsing("Add a custom target", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut self.new_target_name);
                });
                dir_picker(ui, &mut self.new_target_path);
                let ok = !self.new_target_name.trim().is_empty()
                    && !self.new_target_path.trim().is_empty();
                if ui
                    .add_enabled(ok, egui::Button::new("Add target"))
                    .clicked()
                {
                    self.add_custom_target();
                }
            });

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

    fn draw_folder_sizes(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy();

        card(ui, "Scan setup", |ui| {
            dir_picker(ui, &mut self.folder_sizes.dir_path);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if primary_button(ui, !busy, "Analyze").clicked() {
                    self.start_folder_sizes_scan();
                }
            });
            ui.label(
                egui::RichText::new(
                    "Totals up each top-level subfolder so you can see what's taking the space.",
                )
                .weak()
                .small(),
            );
        });

        ui.add_space(10.0);

        card(ui, "Results", |ui| {
            if self.folder_sizes.entries.is_empty() {
                empty_state(ui, "No breakdown yet — run an analysis.");
                return;
            }
            ui.label(format!(
                "{} across {} top-level entries",
                helpers::human_size(self.folder_sizes.total),
                self.folder_sizes.entries.len()
            ));
            ui.add_space(6.0);
            let max = self
                .folder_sizes
                .entries
                .first()
                .map(|e| e.size)
                .unwrap_or(1)
                .max(1);
            let total = self.folder_sizes.total.max(1);
            egui::ScrollArea::vertical()
                .id_source("folder_sizes_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for e in &self.folder_sizes.entries {
                        let share = e.size as f32 / max as f32;
                        let pct = e.size as f64 / total as f64 * 100.0;
                        ui.add(
                            egui::ProgressBar::new(share)
                                .fill(accent(ui))
                                .text(format!(
                                    "{} — {} ({:.0}%)",
                                    e.name,
                                    helpers::human_size(e.size),
                                    pct
                                )),
                        );
                        ui.add_space(2.0);
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
        self.maybe_run_scheduled();
        if self.settings.schedule_enabled {
            // Wake up periodically so due scans fire even when idle.
            ctx.request_repaint_after(std::time::Duration::from_secs(30));
        }

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
                if nav_item(ui, self.tab, Tab::Dashboard, Glyph::House, "Dashboard") {
                    self.tab = Tab::Dashboard;
                }
                if nav_item(ui, self.tab, Tab::FolderSizes, Glyph::Chart, "Folder Sizes") {
                    self.tab = Tab::FolderSizes;
                }
                if nav_item(ui, self.tab, Tab::Storage, Glyph::Disk, "Storage") {
                    self.tab = Tab::Storage;
                }

                ui.add_space(8.0);
                nav_section(ui, "CLEANING");
                if nav_item(ui, self.tab, Tab::CustomClean, Glyph::Broom, "Custom Clean") {
                    self.tab = Tab::CustomClean;
                }
                if nav_item(ui, self.tab, Tab::Duplicates, Glyph::Copy, "Duplicates") {
                    self.tab = Tab::Duplicates;
                }
                if nav_item(ui, self.tab, Tab::LargeFiles, Glyph::File, "Large Files") {
                    self.tab = Tab::LargeFiles;
                }
                if nav_item(ui, self.tab, Tab::SystemCleaner, Glyph::Screen, "System Cleaner") {
                    self.tab = Tab::SystemCleaner;
                }
                if nav_item(ui, self.tab, Tab::EmptyFolders, Glyph::Trash, "Empty Folders") {
                    self.tab = Tab::EmptyFolders;
                }

                ui.add_space(8.0);
                nav_section(ui, "APP");
                if nav_item(ui, self.tab, Tab::Changelog, Glyph::List, "Changelog") {
                    self.tab = Tab::Changelog;
                }
                if nav_item(ui, self.tab, Tab::About, Glyph::Info, "About") {
                    self.tab = Tab::About;
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        let (r, _) = ui.allocate_exact_size(
                            egui::vec2(110.0, 18.0),
                            egui::Sense::hover(),
                        );
                        let (col, g, txt) = if self.settings.dry_run {
                            (warn_color(), Glyph::Eye, "Dry run on")
                        } else {
                            (
                                egui::Color32::from_rgb(0x4C, 0xAF, 0x50),
                                Glyph::Check,
                                "Live mode",
                            )
                        };
                        let p = ui.painter().with_clip_rect(r);
                        paint_glyph(
                            &p,
                            egui::pos2(r.min.x + 8.0, r.center().y),
                            6.5,
                            g,
                            col,
                            ui.visuals().panel_fill,
                        );
                        p.text(
                            egui::pos2(r.min.x + 20.0, r.center().y),
                            egui::Align2::LEFT_CENTER,
                            txt,
                            egui::FontId::proportional(11.5),
                            col,
                        );
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
                Tab::FolderSizes => self.draw_folder_sizes(ui),
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

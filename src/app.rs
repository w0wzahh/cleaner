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
use std::time::Instant;

use chrono::{Local, TimeZone};
use eframe::egui;
use rfd::FileDialog;
use std::io::Write;

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::helpers;
use crate::models::*;
use crate::settings;
use crate::themes;
use crate::workers;

// -----------------------------------------------------------------------------
// shared UI helpers
// -----------------------------------------------------------------------------

/// Percent-encode a string for use inside a mailto URL.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Lowercased file name for case-insensitive name sorting.
fn name_key(p: &Path) -> String {
    p.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn accent(ui: &egui::Ui) -> egui::Color32 {
    ui.visuals().selection.bg_fill
}

fn danger_color() -> egui::Color32 {
    egui::Color32::from_rgb(0xC0, 0x39, 0x2B)
}

fn warn_color() -> egui::Color32 {
    egui::Color32::from_rgb(0xE0, 0x9A, 0x2E)
}

/// Color for changelog section headers — matches GitHub's label colors
/// (green = added, amber = fixed, red = removed, purple = security,
/// blue = changed/improved).
fn changelog_section_color(title: &str) -> egui::Color32 {
    let t = title.to_lowercase();
    if t.contains("add") {
        egui::Color32::from_rgb(0x3F, 0xB9, 0x50)
    } else if t.contains("fix") {
        egui::Color32::from_rgb(0xD2, 0x99, 0x22)
    } else if t.contains("remov") {
        egui::Color32::from_rgb(0xF8, 0x51, 0x49)
    } else if t.contains("secur") {
        egui::Color32::from_rgb(0xA3, 0x71, 0xF7)
    } else {
        egui::Color32::from_rgb(0x58, 0xA6, 0xFF)
    }
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

/// Full-width primary action — the scan button at the bottom of setup cards.
fn primary_button_fill(
    ui: &mut egui::Ui,
    enabled: bool,
    text: impl Into<String>,
) -> egui::Response {
    let w = ui.available_width();
    ui.add_enabled(
        enabled,
        egui::Button::new(
            egui::RichText::new(text.into())
                .strong()
                .color(egui::Color32::WHITE),
        )
        .fill(accent(ui))
        .min_size(egui::vec2(w, 30.0)),
    )
}

/// Compact stat chip used in result-card headers ("247 files", "1.2 GB").
fn stat_chip(ui: &mut egui::Ui, value: impl Into<String>, note: &str) {
    egui::Frame::none()
        .fill(ui.visuals().extreme_bg_color)
        .inner_margin(egui::Margin::symmetric(10.0, 5.0))
        .rounding(egui::Rounding::same(6.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(value.into()).strong().small());
                if !note.is_empty() {
                    ui.label(egui::RichText::new(note).weak().small());
                }
            });
        });
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

/// Scrollable list of matched files with right-aligned sizes. Virtualized
/// via show_rows — a system scan can match tens of thousands of files and
/// laying out every row every frame would make the UI crawl.
fn matched_file_rows(ui: &mut egui::Ui, files: &[&MatchedFile], id: &str) {
    if files.is_empty() {
        egui::ScrollArea::vertical()
            .id_source(id)
            .auto_shrink([false, false])
            .show(ui, |ui| empty_state(ui, "Nothing here yet — run a scan."));
        return;
    }
    let row_h = ui.text_style_height(&egui::TextStyle::Monospace) + 8.0;
    egui::ScrollArea::vertical()
        .id_source(id)
        .auto_shrink([false, false])
        .show_rows(ui, row_h, files.len(), |ui, range| {
            for f in &files[range] {
                ui.horizontal(|ui| {
                    let resp = ui.monospace(f.path.display().to_string());
                    file_row_menu(&resp, &f.path);
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

/// Reorder each duplicate group so the newest (or oldest) copy lands at
/// index 0 — the slot the UI marks "keep" — then select the rest for
/// deletion. Files whose metadata can't be read stay put.
fn dupes_select_keeping(groups: &mut [DuplicateGroup], keep_newest: bool) -> Vec<PathBuf> {
    for g in groups.iter_mut() {
        let mut keep = 0usize;
        let mut best: Option<std::time::SystemTime> = None;
        for (i, p) in g.files.iter().enumerate() {
            if let Ok(t) = fs::metadata(p).and_then(|m| m.modified()) {
                let better = match best {
                    None => true,
                    Some(b) => {
                        if keep_newest {
                            t > b
                        } else {
                            t < b
                        }
                    }
                };
                if better {
                    best = Some(t);
                    keep = i;
                }
            }
        }
        g.files.swap(0, keep);
    }
    groups
        .iter()
        .flat_map(|g| g.files.iter().skip(1).cloned())
        .collect()
}

/// Row-level file interactions: double-click reveals the file, right-click
/// offers Copy path / Reveal in Explorer.
fn file_row_menu(resp: &egui::Response, path: &Path) {
    if resp.double_clicked() {
        reveal_in_explorer(path);
    }
    resp.context_menu(|ui| {
        if ui.button("Copy path").clicked() {
            ui.ctx().output_mut(|o| o.copied_text = path.display().to_string());
            ui.close_menu();
        }
        if ui.button("Reveal in Explorer").clicked() {
            reveal_in_explorer(path);
            ui.close_menu();
        }
    });
}

/// Open a path in the system file manager. `explorer /select` highlights the
/// file or folder inside its parent instead of opening it.
fn reveal_in_explorer(path: &Path) {
    if cfg!(windows) && path.exists() {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,\"{}\"", path.display()))
            .spawn();
    } else {
        let _ = open::that(path);
    }
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

/// Human label for a schedule interval in hours. Values outside the preset
/// list (e.g. a hand-edited settings file) still get an honest label.
fn schedule_interval_label(hours: u32) -> String {
    match hours {
        1 => "1 hour".to_string(),
        6 => "6 hours".to_string(),
        12 => "12 hours".to_string(),
        24 => "24 hours (daily)".to_string(),
        168 => "7 days (weekly)".to_string(),
        h => format!("every {} hours", h),
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
    /// Log lines waiting to be appended to the history file — flushed once
    /// per frame by `flush_log` instead of one file open per line.
    pub log_pending: Vec<String>,
    pub rx: Option<mpsc::Receiver<workers::WorkerMessage>>,
    pub status: String,
    pub status_toast: u64,
    pub confirm_action: Option<ConfirmAction>,
    pub confirm_title: String,
    pub confirm_body: String,
    pub theme_anim: themes::ThemeAnim,
    pub pending_theme_change: Option<themes::Theme>,
    /// Which tab a scheduled auto-clean should act on once its scan finishes.
    pub pending_auto_clean: Option<Tab>,
    /// Paths the last clean actually deleted (from the worker's CleanStats).
    pub last_cleaned_paths: Vec<PathBuf>,
    pub initial_theme_applied: bool,

    /// System-tray icon (always present while the app runs). The icon handle
    /// is only held so it isn't dropped; events are handled by a watcher thread.
    pub tray: Option<TrayIcon>,
    /// Set once the tray + watcher thread have been created (or failed).
    pub tray_setup_done: bool,
    /// Cached "is the Windows scheduled task registered" state.
    pub task_registered: Option<bool>,
    /// Last tooltip text pushed to the tray icon — only set on change.
    pub tray_tooltip: String,
    /// Last title pushed to the window — only set on change.
    pub window_title: String,
    /// When the current scan/clean started — used for elapsed-time logging.
    pub op_started: Option<Instant>,
    /// Park the window in the tray on the first frame (setting or
    /// `--minimized` launch flag). Consumed once the tray is ready.
    pub start_in_tray: bool,
    /// Whether the theme-customization window is open.
    pub customize_open: bool,
    /// Version selected in the changelog viewer (None = latest).
    pub changelog_version: Option<String>,
    /// App-icon texture for the About page hero (loaded once).
    pub icon_tex: Option<egui::TextureHandle>,

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
        let initial_visuals =
            themes::tinted_visuals(settings.theme, settings.accent_rgb);
        let mut app = Self {
            tab: Tab::Dashboard,
            settings,
            scanning: false,
            cleaning: false,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            progress: 0.0,
            log: Vec::new(),
            log_pending: Vec::new(),
            rx: None,
            status: "Ready".to_string(),
            status_toast: 0,
            confirm_action: None,
            confirm_title: "Confirm action".to_string(),
            confirm_body: String::new(),
            theme_anim: themes::ThemeAnim::new(initial_visuals),
            initial_theme_applied: false,
            pending_theme_change: None,
            pending_auto_clean: None,
            last_cleaned_paths: Vec::new(),
            tray: None,
            tray_setup_done: false,
            task_registered: None,
            tray_tooltip: "Cleaner".to_string(),
            window_title: "Cleaner".to_string(),
            op_started: None,
            start_in_tray: false,
            customize_open: false,
            changelog_version: None,
            icon_tex: None,
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
        // Restore persisted scan filters.
        app.custom.extensions = app.settings.custom_extensions.clone();
        app.custom.pattern = app.settings.custom_pattern.clone();
        app.custom.older_than_days = app.settings.custom_older_than;
        app.custom.min_size_bytes = app.settings.custom_min_size;
        app.custom.max_size_bytes = app.settings.custom_max_size;
        app.custom.exclude_dirs = app.settings.custom_exclude_dirs.clone();
        app.large_files.threshold_mb = app.settings.large_threshold_mb;
        // Per-tab directories override the shared default when persisted.
        if !app.settings.dir_custom.is_empty() {
            app.custom.dir_path = app.settings.dir_custom.clone();
        }
        if !app.settings.dir_duplicates.is_empty() {
            app.duplicates.dir_path = app.settings.dir_duplicates.clone();
        }
        if !app.settings.dir_large_files.is_empty() {
            app.large_files.dir_path = app.settings.dir_large_files.clone();
        }
        if !app.settings.dir_empty_folders.is_empty() {
            app.empty_folders.dir_path = app.settings.dir_empty_folders.clone();
        }
        if !app.settings.dir_folder_sizes.is_empty() {
            app.folder_sizes.dir_path = app.settings.dir_folder_sizes.clone();
        }
        app.start_in_tray = app.settings.start_minimized;
        app.rebuild_custom_targets();
        app
    }
}

impl CleanerApp {
    /// Create the system-tray icon with a Show/Quit menu, then spawn a
    /// dedicated watcher thread for tray events.
    ///
    /// The watcher runs on its own thread because `update()` stops being
    /// called once the window is hidden — polling from inside the frame loop
    /// would never see the "Show" click. `egui::Context` is cheap to clone
    /// and `send_viewport_cmd`/`request_repaint` are safe from any thread.
    /// Failure is non-fatal — the app works fine without a tray icon.
    fn setup_tray(&mut self, ctx: &egui::Context) {
        self.tray_setup_done = true;
        let icon = themes::generate_icon();
        let Ok(tray_icon_img) =
            tray_icon::Icon::from_rgba(icon.rgba.clone(), icon.width, icon.height)
        else {
            return;
        };
        let show = MenuItem::new("Show Cleaner", true, None);
        let quit = MenuItem::new("Quit", true, None);
        let show_id = show.id().clone();
        let quit_id = quit.id().clone();
        let menu = Menu::new();
        let _ = menu.append(&show);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit);
        match TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Cleaner")
            .with_icon(tray_icon_img)
            .build()
        {
            Ok(tray) => self.tray = Some(tray),
            Err(_) => return,
        }

        let ctx = ctx.clone();
        thread::spawn(move || {
            let mut ticks = 0u32;
            loop {
            let mut restore = false;
            for event in TrayIconEvent::receiver().try_iter() {
                if let TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } = event
                {
                    restore = true;
                }
            }
            for event in MenuEvent::receiver().try_iter() {
                if event.id == show_id {
                    restore = true;
                } else if event.id == quit_id {
                    // WM_CLOSE → winit close event → clean egui shutdown.
                    // ViewportCommand::Close as a fallback if the HWND was
                    // never captured.
                    helpers::close_main_window();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
            }
            if restore {
                helpers::show_main_window();
                ctx.request_repaint();
            }
            // While the window is hidden, update() gets no repaint events —
            // nudge it periodically so scheduled scans still fire and worker
            // results still get processed in the tray.
            ticks += 1;
            if ticks % 600 == 0 {
                ctx.request_repaint();
            }
            thread::sleep(std::time::Duration::from_millis(50));
            }
        });
    }

    /// Hide the window to the tray.
    fn hide_to_tray(&mut self, ctx: &egui::Context) {
        if self.tray.is_none() {
            self.add_log("Tray icon unavailable on this system.");
            return;
        }
        // Park the window off-screen via user32 — NOT ViewportCommand or
        // SW_HIDE: a hidden window gets no repaint events, which starves
        // update() and would freeze scheduled scans and worker polling.
        if helpers::have_main_hwnd() {
            helpers::hide_main_window();
        } else {
            // No HWND captured — tray restore won't work either, but at
            // least hide the window so the button isn't a no-op.
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        ctx.request_repaint();
    }
}

// -----------------------------------------------------------------------------
// logging + status
// -----------------------------------------------------------------------------

impl CleanerApp {
    /// Append to the visible log + status line and queue a disk write.
    /// Disk writes are batched in `flush_log` — a 50k-file clean produces a
    /// log line per file, and opening the history file per line would make
    /// the UI thread do 50k file opens.
    pub fn add_log(&mut self, msg: &str) {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        let line = format!("[{}] {}", timestamp, msg);
        self.log.push(line.clone());
        if self.log.len() > 1200 {
            // Bulk-drain keeps this amortized O(1) — Vec::remove(0) shifts
            // the whole buffer on every call once the cap is hit.
            self.log.drain(..200);
        }
        if self.status_toast == 0 {
            self.status = msg.to_string();
            self.status_toast = 60;
        }
        self.log_pending.push(line);
    }

    /// Write queued log lines to the history file in one open. Called at
    /// the end of `update` and `on_exit` so a frame's worth of lines is a
    /// single append, and ordering is preserved exactly.
    pub fn flush_log(&mut self) {
        if self.log_pending.is_empty() {
            return;
        }
        let lines = std::mem::take(&mut self.log_pending);
        let log_path = self.settings.log_path();
        // Rotate when the log grows past ~1 MB so it can't grow forever.
        if let Ok(meta) = fs::metadata(&log_path) {
            if meta.len() > 1_048_576 {
                let _ = fs::rename(&log_path, log_path.with_extension("old.log"));
            }
        }
        if let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            for line in &lines {
                let _ = writeln!(file, "{}", line);
            }
        }
    }

    /// Sync user-defined system-clean targets from settings into the UI list,
    /// then re-apply which built-ins the user has unchecked.
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
        for t in &mut self.system.targets {
            if !t.custom {
                t.enabled = !self.settings.disabled_targets.contains(&t.name);
            }
        }
    }

    fn add_custom_target(&mut self) {
        let name = self.new_target_name.trim().to_string();
        let path = self.new_target_path.trim().to_string();
        if name.is_empty() || path.is_empty() {
            return;
        }
        let pb = PathBuf::from(&path);
        if !pb.is_dir() {
            self.add_log(&format!(
                "Custom target not added — folder doesn't exist: {}",
                path
            ));
            self.status = "Folder not found".to_string();
            self.status_toast = 60;
            return;
        }
        if self
            .settings
            .custom_targets
            .iter()
            .any(|c| PathBuf::from(&c.path) == pb)
        {
            self.add_log("That folder is already a cleaning target.");
            self.status = "Already added".to_string();
            self.status_toast = 60;
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
    /// Theme visuals with the user's accent override applied.
    fn effective_visuals(&self) -> egui::Visuals {
        themes::tinted_visuals(self.settings.theme, self.settings.accent_rgb)
    }

    /// Secondary accent — user override, else theme default.
    fn accent2(&self) -> egui::Color32 {
        self.settings
            .accent2_rgb
            .map(|[r, g, b]| egui::Color32::from_rgb(r, g, b))
            .unwrap_or_else(|| self.settings.theme.accent2())
    }

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
        let to = themes::tinted_visuals(new_theme, self.settings.accent_rgb);
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
        let auto = self.settings.schedule_auto_clean;
        match self.settings.schedule_target {
            crate::settings::ScheduleTarget::System => {
                self.tab = Tab::SystemCleaner;
                self.pending_auto_clean = auto.then_some(Tab::SystemCleaner);
                self.start_system_scan();
            }
            crate::settings::ScheduleTarget::Custom => {
                // Never fall back to the Custom tab's directory here — it
                // defaults to the user's home folder, which would make a
                // scheduled (possibly auto-cleaning) run sweep the whole
                // profile. No configured folder means skip.
                if self.settings.schedule_dir.trim().is_empty() {
                    self.add_log(
                        "Scheduled scan skipped — pick a folder in the Scheduler card.",
                    );
                    return;
                }
                self.tab = Tab::CustomClean;
                self.pending_auto_clean = auto.then_some(Tab::CustomClean);
                self.custom.dir_path = self.settings.schedule_dir.clone();
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
        // Persist the current filters so they survive restarts.
        self.settings.custom_extensions = self.custom.extensions.clone();
        self.settings.custom_pattern = self.custom.pattern.clone();
        self.settings.custom_older_than = self.custom.older_than_days;
        self.settings.custom_min_size = self.custom.min_size_bytes;
        self.settings.custom_max_size = self.custom.max_size_bytes;
        self.settings.custom_exclude_dirs = self.custom.exclude_dirs.clone();
        self.settings.dir_custom = self.custom.dir_path.clone();
        self.settings.save();
        self.scanning = true;
        self.op_started = Some(Instant::now());
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
        self.settings.dir_duplicates = self.duplicates.dir_path.clone();
        self.settings.save();
        self.scanning = true;
        self.op_started = Some(Instant::now());
        self.progress = 0.0;
        self.duplicates.groups.clear();
        self.duplicates.selected_files.clear();
        self.duplicates.total_wasted = 0;
        self.log.clear();
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let dir = self.duplicates.dir_path.clone();
        // Protected paths apply everywhere; dupe excludes only here.
        let mut excludes = self.settings.protected_list();
        excludes.extend(self.settings.dupe_exclude_list());
        self.add_log("Starting duplicate scan...");
        thread::spawn(move || workers::duplicates_worker(dir, excludes, cancel, tx));
        self.rx = Some(rx);
        self.status = "Scanning duplicates...".to_string();
        self.status_toast = 0;
    }

    pub fn start_large_files_scan(&mut self) {
        if self.busy() {
            return;
        }
        self.settings.large_threshold_mb = self.large_files.threshold_mb;
        self.settings.dir_large_files = self.large_files.dir_path.clone();
        self.settings.save();
        self.scanning = true;
        self.op_started = Some(Instant::now());
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
        let protected = self.settings.protected_list();
        self.add_log("Starting large file scan...");
        thread::spawn(move || {
            workers::large_files_worker(dir, threshold, protected, cancel, tx)
        });
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
        self.op_started = Some(Instant::now());
        self.progress = 0.0;
        self.system.matched_files.clear();
        self.system.total_matched_size = 0;
        self.log.clear();
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let targets = self.system.targets.clone();
        let protected = self.settings.protected_list();
        self.add_log("Starting system scan...");
        thread::spawn(move || workers::system_scan_worker(targets, protected, cancel, tx));
        self.rx = Some(rx);
        self.status = "Scanning system...".to_string();
        self.status_toast = 0;
    }

    pub fn start_empty_folders_scan(&mut self) {
        if self.busy() {
            return;
        }
        self.settings.dir_empty_folders = self.empty_folders.dir_path.clone();
        self.settings.save();
        self.scanning = true;
        self.op_started = Some(Instant::now());
        self.progress = 0.0;
        self.empty_folders.folders.clear();
        self.log.clear();
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let dir = self.empty_folders.dir_path.clone();
        let protected = self.settings.protected_list();
        self.add_log("Starting empty folder scan...");
        thread::spawn(move || workers::empty_folders_worker(dir, protected, cancel, tx));
        self.rx = Some(rx);
        self.status = "Scanning for empty folders...".to_string();
        self.status_toast = 0;
    }

    pub fn start_folder_sizes_scan(&mut self) {
        if self.busy() {
            return;
        }
        self.settings.dir_folder_sizes = self.folder_sizes.dir_path.clone();
        self.settings.save();
        self.scanning = true;
        self.op_started = Some(Instant::now());
        self.progress = 0.0;
        self.folder_sizes.entries.clear();
        self.folder_sizes.total = 0;
        self.log.clear();
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let cancel = self.cancel_flag.clone();
        let dir = self.folder_sizes.dir_path.clone();
        self.add_log("Starting folder size analysis...");
        let protected = self.settings.protected_list();
        thread::spawn(move || workers::folder_sizes_worker(dir, protected, cancel, tx));
        self.rx = Some(rx);
        self.status = "Analyzing folder sizes...".to_string();
        self.status_toast = 0;
    }

    pub fn start_clean_selected(&mut self, tab: Tab) {
        if self.busy() {
            return;
        }
        let files: Vec<MatchedFile> = match tab {
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
        self.op_started = Some(Instant::now());
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
        self.op_started = Some(Instant::now());
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
        self.op_started = Some(Instant::now());
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
                ConfirmAction::CleanFiles(tab) => self.start_clean_selected(tab),
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

    /// Drop results that were actually deleted so the UI doesn't offer them
    /// twice. Uses the worker's reported paths — files that failed to delete
    /// stay in the list so they can be retried. Prunes every result list so
    /// switching tabs mid-clean can't resurrect stale entries.
    fn prune_after_clean(&mut self) {
        if self.last_cleaned_paths.is_empty() {
            return;
        }
        let gone: std::collections::HashSet<PathBuf> =
            self.last_cleaned_paths.iter().cloned().collect();

        self.custom.matched_files.retain(|f| !gone.contains(&f.path));
        self.custom.selected.retain(|p| !gone.contains(p));
        self.custom.total_matched_size =
            self.custom.matched_files.iter().map(|f| f.size).sum();

        self.system.matched_files.retain(|f| !gone.contains(&f.path));
        self.system.total_matched_size =
            self.system.matched_files.iter().map(|f| f.size).sum();

        self.large_files.files.retain(|f| !gone.contains(&f.path));
        self.large_files.selected.retain(|p| !gone.contains(p));
        self.large_files.total_size =
            self.large_files.files.iter().map(|f| f.size).sum();

        self.duplicates.selected_files.retain(|p| !gone.contains(p));
        for g in &mut self.duplicates.groups {
            g.files.retain(|f| !gone.contains(f));
        }
        self.duplicates.groups.retain(|g| g.files.len() > 1);
        self.duplicates.total_wasted = self.dup_wasted();

        self.empty_folders.folders.retain(|p| !gone.contains(p));
    }

    pub fn poll_messages(&mut self, ctx: &egui::Context) {
        if let Some(rx) = self.rx.take() {
            let mut still_active = true;
            loop {
                let msg = match rx.try_recv() {
                    Ok(m) => m,
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // The worker thread died without a Done/Error (e.g. a
                        // panic) — clear the busy flags or the UI is stuck.
                        if self.scanning || self.cleaning {
                            self.add_log("ERROR: worker stopped unexpectedly.");
                            self.status = "Error".to_string();
                            self.status_toast = 120;
                            self.scanning = false;
                            self.cleaning = false;
                            self.pending_auto_clean = None;
                        }
                        still_active = false;
                        break;
                    }
                };
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
                    workers::WorkerMessage::CleanStats {
                        deleted,
                        freed,
                        paths,
                    } => {
                        self.last_cleaned_paths = paths;
                        // A dry-run clean reports what *would* be deleted —
                        // don't inflate lifetime stats or the activity chart.
                        if !self.settings.dry_run {
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
                    }
                    workers::WorkerMessage::Done { summary } => {
                        let was_scanning = self.scanning;
                        let was_cleaning = self.cleaning;
                        let elapsed = self
                            .op_started
                            .take()
                            .map(|t| format!(" (took {:.1}s)", t.elapsed().as_secs_f32()))
                            .unwrap_or_default();
                        self.add_log(&format!("{}{}", summary, elapsed));
                        self.status = if was_scanning {
                            "Scan complete".to_string()
                        } else {
                            "Clean complete".to_string()
                        };
                        self.status_toast = 120;
                        // Clear busy flags BEFORE a possible auto-clean —
                        // start_clean_selected installs its own flag and a
                        // fresh channel on self.rx, which must not be
                        // clobbered by the cleanup below.
                        self.scanning = false;
                        self.cleaning = false;
                        self.progress = 0.0;
                        if was_scanning {
                            self.last_scan_summary = summary;
                            if let Some(tab) = self.pending_auto_clean.take() {
                                if self.settings.dry_run {
                                    self.add_log(
                                        "Scheduled auto-clean skipped — dry run is on.",
                                    );
                                } else {
                                    self.add_log("Scheduled auto-clean starting...");
                                    self.start_clean_selected(tab);
                                }
                            }
                        }
                        if was_cleaning && !self.settings.dry_run {
                            self.prune_after_clean();
                        }
                        // Nudge the user if the window isn't focused.
                        helpers::flash_main_window();
                        still_active = false;
                    }
                    workers::WorkerMessage::Error(e) => {
                        self.add_log(&format!("ERROR: {}", e));
                        self.status = "Error".to_string();
                        self.status_toast = 120;
                        helpers::flash_main_window();
                        self.scanning = false;
                        self.cleaning = false;
                        self.op_started = None;
                        still_active = false;
                    }
                    workers::WorkerMessage::Cancelled => {
                        self.add_log("Operation cancelled by user.");
                        self.status = "Cancelled".to_string();
                        self.status_toast = 60;
                        self.scanning = false;
                        self.cleaning = false;
                        self.op_started = None;
                        still_active = false;
                    }
                }
                if !still_active {
                    break;
                }
            }
            if still_active {
                self.rx = Some(rx);
                ctx.request_repaint();
            }
            // When !still_active, self.rx stays whatever it is: None if the
            // worker ended, or the fresh channel an auto-clean just installed
            // — clearing it here would orphan that worker's messages.
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
        // First-run card: explain the safety model once, dismissible.
        if !self.settings.welcomed {
            card(ui, "Getting started", |ui| {
                ui.label("1. Pick a tool on the left — or hit the big ring to scan your system.");
                ui.label("2. Review what it found — nothing is deleted until you say so.");
                ui.label(
                    "3. Dry run is ON by default, so scans only preview. Toggle it off in Safety below when you're ready to clean for real.",
                );
                ui.add_space(4.0);
                if ui.button("Got it — don't show this again").clicked() {
                    self.settings.welcomed = true;
                    self.settings.save();
                }
            });
            ui.add_space(10.0);
        }

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
                            self.accent2(),
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
            if self.settings.schedule_target == crate::settings::ScheduleTarget::Custom {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Folder:").weak().small(),
                    );
                    let mut d = self.settings.schedule_dir.clone();
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut d)
                                .hint_text("folder to scan (required)")
                                .desired_width(280.0),
                        )
                        .changed()
                    {
                        self.settings.schedule_dir = d;
                        self.settings.save();
                    }
                    if ui.small_button("Browse…").clicked() {
                        if let Some(p) = FileDialog::new().pick_folder() {
                            self.settings.schedule_dir = p.display().to_string();
                            self.settings.save();
                        }
                    }
                });
                if self.settings.schedule_dir.trim().is_empty() {
                    ui.label(
                        egui::RichText::new(
                            "No folder set — scheduled runs are skipped until you pick one.",
                        )
                        .weak()
                        .small(),
                    );
                }
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let registered = self.task_registered.unwrap_or_else(|| {
                    let r = helpers::task_registered();
                    self.task_registered = Some(r);
                    r
                });
                let mut want = registered;
                if ui
                    .checkbox(&mut want, "also run when the app is closed")
                    .on_hover_text(
                        "Registers a Windows Task Scheduler entry (\"CleanerScheduledScan\") that runs the same schedule headlessly, even when Cleaner isn't open. Uncheck to remove it.",
                    )
                    .changed()
                {
                    let is_custom = self.settings.schedule_target
                        == crate::settings::ScheduleTarget::Custom;
                    if want && is_custom && self.settings.schedule_dir.trim().is_empty() {
                        // Registering now would bake the home folder into the
                        // task — a scheduled clean of the whole user profile.
                        self.add_log(
                            "Scheduled task not registered — pick a schedule folder first.",
                        );
                        self.task_registered = Some(false);
                    } else if want {
                        let args = helpers::task_command_args(
                            is_custom,
                            self.settings.schedule_auto_clean,
                            &self.settings.schedule_dir,
                        );
                        match helpers::register_task(self.settings.schedule_hours, &args) {
                            Ok(()) => {
                                self.task_registered = Some(true);
                                self.add_log("Windows scheduled task registered.");
                            }
                            Err(e) => {
                                self.task_registered = Some(false);
                                self.add_log(&format!(
                                    "Couldn't register scheduled task: {}",
                                    e
                                ));
                            }
                        }
                    } else {
                        match helpers::unregister_task() {
                            Ok(()) => {
                                self.task_registered = Some(false);
                                self.add_log("Windows scheduled task removed.");
                            }
                            Err(e) => {
                                self.add_log(&format!(
                                    "Couldn't remove scheduled task: {}",
                                    e
                                ));
                            }
                        }
                    }
                }
                if registered {
                    ui.label(
                        egui::RichText::new("(task registered)")
                            .weak()
                            .small(),
                    );
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

        card(ui, "Startup", |ui| {
            if ui
                .checkbox(
                    &mut self.settings.start_minimized,
                    "Start minimized to the system tray",
                )
                .on_hover_text(
                    "The window opens parked in the tray instead of on screen. \
                     Applies the next time Cleaner launches.",
                )
                .changed()
            {
                self.settings.save();
            }
            let mut want_start = self.settings.run_at_startup;
            if ui
                .checkbox(&mut want_start, "Run Cleaner when Windows starts")
                .on_hover_text(
                    "Adds a per-user Run-key entry — no admin needed. \
                     Uses the \"start minimized\" setting above.",
                )
                .changed()
            {
                match helpers::set_startup(want_start, self.settings.start_minimized) {
                    Ok(()) => {
                        self.settings.run_at_startup = want_start;
                        self.settings.save();
                        self.add_log(if want_start {
                            "Cleaner will launch when Windows starts."
                        } else {
                            "Startup entry removed."
                        });
                    }
                    Err(e) => {
                        self.add_log(&format!("Couldn't update startup entry: {}", e));
                    }
                }
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
                    .stick_to_bottom(true)
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
                    if ui.small_button("Copy").clicked() {
                        ui.output_mut(|o| o.copied_text = self.log.join("\n"));
                    }
                    if ui.small_button("Clear").clicked() {
                        self.log.clear();
                    }
                });
            }
        });
    }

    /// Height the results list should occupy so the footer action row stays
    /// pinned to the bottom of the results card.
    fn list_area_h(ui: &egui::Ui) -> f32 {
        let avail = ui.available_height();
        if avail.is_finite() {
            (avail - 46.0).max(80.0)
        } else {
            420.0
        }
    }

    /// Two-column scan layout: a fixed-width setup column on the left, the
    /// results card filling the rest — the arrangement pro cleaners use.
    /// Falls back to a single scrollable column on narrow windows.
    fn scan_page(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        setup: impl FnOnce(&mut Self, &mut egui::Ui),
        results: impl FnOnce(&mut Self, &mut egui::Ui),
    ) {
        const SETUP_W: f32 = 300.0;
        const GAP: f32 = 10.0;
        if ui.available_width() < SETUP_W + 420.0 {
            egui::ScrollArea::vertical()
                .id_source(format!("{}_page", id))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    setup(self, ui);
                    ui.add_space(GAP);
                    results(self, ui);
                });
            return;
        }
        let h = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(SETUP_W, h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_source(format!("{}_setup", id))
                        .auto_shrink([false, false])
                        .show(ui, |ui| setup(self, ui));
                },
            );
            ui.add_space(GAP);
            let w = ui.available_width().max(120.0);
            ui.allocate_ui_with_layout(
                egui::vec2(w, h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| results(self, ui),
            );
        });
    }

    fn draw_custom_clean(&mut self, ui: &mut egui::Ui) {
        self.scan_page(
            ui,
            "custom",
            |s, ui| {
                card(ui, "Scan setup", |ui| {
                    dir_picker(ui, &mut s.custom.dir_path);
                    ui.add_space(6.0);
                    egui::Grid::new("custom_filters")
                        .num_columns(2)
                        .spacing([10.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Extensions");
                            ui.add(
                                egui::TextEdit::singleline(&mut s.custom.extensions)
                                    .hint_text("tmp, log")
                                    .desired_width(f32::INFINITY),
                            );
                            ui.end_row();

                            ui.label("Glob pattern");
                            ui.add(
                                egui::TextEdit::singleline(&mut s.custom.pattern)
                                    .hint_text("*.tmp")
                                    .desired_width(f32::INFINITY),
                            );
                            ui.end_row();

                            ui.label("Older than (days)");
                            ui.add(
                                egui::Slider::new(&mut s.custom.older_than_days, 0..=36500)
                                    .text("days"),
                            );
                            ui.end_row();

                            ui.label("Min size (bytes)");
                            ui.add(
                                egui::DragValue::new(&mut s.custom.min_size_bytes)
                                    .speed(1000),
                            );
                            ui.end_row();

                            ui.label("Max size (bytes)");
                            ui.add(
                                egui::DragValue::new(&mut s.custom.max_size_bytes)
                                    .speed(1000),
                            );
                            ui.end_row();

                            ui.label("Exclude dirs");
                            ui.add(
                                egui::TextEdit::singleline(&mut s.custom.exclude_dirs)
                                    .hint_text("comma separated")
                                    .desired_width(f32::INFINITY),
                            );
                            ui.end_row();
                        });
                    ui.label(
                        egui::RichText::new("Max size 0 = no limit. Excludes are absolute paths.")
                            .weak()
                            .small(),
                    );

                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Presets").weak().small());
                    ui.add_space(2.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui.small_button("Temp & logs").clicked() {
                            s.custom.extensions = "tmp,log,bak,dmp".to_string();
                            s.custom.pattern.clear();
                            s.custom.older_than_days = 0;
                            s.custom.min_size_bytes = 0;
                            s.custom.max_size_bytes = 0;
                        }
                        if ui.small_button("Old files (30d+)").clicked() {
                            s.custom.older_than_days = 30;
                            s.custom.extensions.clear();
                            s.custom.pattern.clear();
                            s.custom.min_size_bytes = 0;
                            s.custom.max_size_bytes = 0;
                        }
                        if ui.small_button("Big media (50MB+)").clicked() {
                            s.custom.extensions =
                                "mp4,mkv,avi,mov,mp3,flac,wav".to_string();
                            s.custom.min_size_bytes = 50 * 1024 * 1024;
                            s.custom.max_size_bytes = 0;
                            s.custom.older_than_days = 0;
                            s.custom.pattern.clear();
                        }
                        if ui.small_button("Images").clicked() {
                            s.custom.extensions =
                                "png,jpg,jpeg,gif,bmp,webp".to_string();
                            s.custom.pattern.clear();
                            s.custom.older_than_days = 0;
                            s.custom.min_size_bytes = 0;
                            s.custom.max_size_bytes = 0;
                        }
                        if ui.small_button("Old Downloads").clicked() {
                            if let Some(d) = dirs::download_dir() {
                                s.custom.dir_path = d.display().to_string();
                            }
                            s.custom.older_than_days = 30;
                            s.custom.extensions.clear();
                            s.custom.pattern.clear();
                            s.custom.min_size_bytes = 0;
                            s.custom.max_size_bytes = 0;
                        }
                    });

                    ui.add_space(10.0);
                    if primary_button_fill(ui, !s.busy(), "Scan folder")
                        .on_hover_text("Scan the folder using the filters above")
                        .clicked()
                    {
                        s.start_custom_scan();
                    }
                });
            },
            |s, ui| {
                let busy = s.busy();
                card(ui, "Results", |ui| {
                    ui.horizontal_wrapped(|ui| {
                        stat_chip(
                            ui,
                            format!("{}", s.custom.matched_files.len()),
                            "files matched",
                        );
                        stat_chip(
                            ui,
                            helpers::human_size(s.custom.total_matched_size),
                            "matched",
                        );
                        stat_chip(
                            ui,
                            format!("{}", s.custom.selected.len()),
                            "selected",
                        );
                    });

                    // File-type breakdown: which extensions account for the size.
                    if !s.custom.matched_files.is_empty() {
                        let mut by_ext: BTreeMap<String, (u64, u64)> = BTreeMap::new();
                        for f in &s.custom.matched_files {
                            let ext = f
                                .path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|e| e.to_lowercase())
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
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Filter:").weak().small());
                        ui.add(
                            egui::TextEdit::singleline(&mut s.custom.filter)
                                .hint_text("type to filter results")
                                .desired_width(180.0),
                        );
                        if !s.custom.filter.is_empty() && ui.small_button("Clear").clicked() {
                            s.custom.filter.clear();
                        }
                        ui.separator();
                        ui.label(egui::RichText::new("Sort:").weak().small());
                        egui::ComboBox::from_id_source("custom_sort")
                            .selected_text(s.custom.sort.label())
                            .show_ui(ui, |ui| {
                                for sm in SortMode::all() {
                                    ui.selectable_value(&mut s.custom.sort, *sm, sm.label());
                                }
                            });
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .add_enabled(
                                        !busy && !s.custom.selected.is_empty(),
                                        egui::Button::new("Clear sel.").small(),
                                    )
                                    .clicked()
                                {
                                    s.custom.selected.clear();
                                }
                                if ui
                                    .add_enabled(
                                        !busy && !s.custom.matched_files.is_empty(),
                                        egui::Button::new("Select all").small(),
                                    )
                                    .on_hover_text(
                                        "Select all results — respects the active filter",
                                    )
                                    .clicked()
                                {
                                    let flt = s.custom.filter.to_lowercase();
                                    s.custom.selected = s
                                        .custom
                                        .matched_files
                                        .iter()
                                        .filter(|f| {
                                            flt.is_empty()
                                                || f.path
                                                    .to_string_lossy()
                                                    .to_lowercase()
                                                    .contains(&flt)
                                        })
                                        .map(|f| f.path.clone())
                                        .collect();
                                }
                            },
                        );
                    });
                    ui.add_space(4.0);

                    let filter = s.custom.filter.to_lowercase();
                    let sort = s.custom.sort;
                    let files = &s.custom.matched_files;
                    let selected = &mut s.custom.selected;
                    let list_h = Self::list_area_h(ui);
                    if files.is_empty() {
                        ui.allocate_ui(
                            egui::vec2(ui.available_width(), list_h),
                            |ui| {
                                egui::ScrollArea::vertical()
                                    .id_source("custom_scroll")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        empty_state(ui, "Nothing here yet — run a scan.")
                                    });
                            },
                        );
                    } else {
                        // Sort+filter once, then virtualize — huge scans
                        // otherwise lay out every row on every frame.
                        let mut view: Vec<(&MatchedFile, String)> = files
                            .iter()
                            .filter(|f| {
                                filter.is_empty()
                                    || f.path
                                        .to_string_lossy()
                                        .to_lowercase()
                                        .contains(&filter)
                            })
                            .map(|f| (f, name_key(&f.path)))
                            .collect();
                        view.sort_by(|a, b| {
                            sort.compare((a.0.size, a.1.as_str()), (b.0.size, b.1.as_str()))
                        });
                        let row_h =
                            ui.text_style_height(&egui::TextStyle::Monospace) + 8.0;
                        ui.allocate_ui(
                            egui::vec2(ui.available_width(), list_h),
                            |ui| {
                                egui::ScrollArea::vertical()
                                    .id_source("custom_scroll")
                                    .auto_shrink([false, false])
                                    .show_rows(ui, row_h, view.len(), |ui, range| {
                                        for (f, _) in &view[range] {
                                            ui.horizontal(|ui| {
                                                let mut on = selected.contains(&f.path);
                                                if ui.checkbox(&mut on, "").changed() {
                                                    if on {
                                                        selected.insert(f.path.clone());
                                                    } else {
                                                        selected.remove(&f.path);
                                                    }
                                                }
                                                let resp = ui.monospace(
                                                    f.path.display().to_string(),
                                                );
                                                file_row_menu(&resp, &f.path);
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        ui.label(
                                                            egui::RichText::new(
                                                                helpers::human_size(
                                                                    f.size,
                                                                ),
                                                            )
                                                            .weak(),
                                                        );
                                                    },
                                                );
                                            });
                                        }
                                    });
                            },
                        );
                    }

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !s.custom.matched_files.is_empty(),
                                egui::Button::new("Export report"),
                            )
                            .clicked()
                        {
                            match helpers::export_report(&s.custom.matched_files, "custom")
                            {
                                Ok(p) => {
                                    s.status = format!("Report saved: {}", p.display());
                                    s.status_toast = 60;
                                }
                                Err(e) => {
                                    s.status = format!("Export error: {}", e);
                                    s.status_toast = 60;
                                }
                            }
                        }
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                let n = s.custom.selected.len();
                                let sel_size: u64 = s
                                    .custom
                                    .matched_files
                                    .iter()
                                    .filter(|f| s.custom.selected.contains(&f.path))
                                    .map(|f| f.size)
                                    .sum();
                                if danger_button(
                                    ui,
                                    !busy && n > 0,
                                    format!("Clean {} files", n),
                                )
                                .on_hover_text(
                                    "Delete the selected files — honors dry run and recycle bin settings",
                                )
                                .clicked()
                                {
                                    if s.settings.confirm_clean {
                                        s.confirm_title =
                                            "Clean selected files?".to_string();
                                        s.confirm_body = format!(
                                            "Delete the {} selected files ({})?",
                                            n,
                                            helpers::human_size(sel_size)
                                        );
                                        s.confirm_action =
                                            Some(ConfirmAction::CleanFiles(s.tab));
                                    } else {
                                        s.start_clean_selected(s.tab);
                                    }
                                }
                            },
                        );
                    });
                });
            },
        );
    }

    fn draw_duplicates(&mut self, ui: &mut egui::Ui) {
        self.scan_page(
            ui,
            "dupes",
            |s, ui| {
                card(ui, "Scan setup", |ui| {
                    dir_picker(ui, &mut s.duplicates.dir_path);
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(
                            "Two-stage hashing: groups by size, quick-hashes 8 KB, then full SHA-256.",
                        )
                        .weak()
                        .small(),
                    );
                    ui.add_space(6.0);
                    ui.collapsing("Exclude folders", |ui| {
                        ui.label(
                            egui::RichText::new(
                                "Skipped during scans — one per line or comma-separated.",
                            )
                            .weak()
                            .small(),
                        );
                        if ui
                            .add(
                                egui::TextEdit::multiline(&mut s.settings.dupe_excludes)
                                    .desired_rows(3)
                                    .desired_width(f32::INFINITY)
                                    .hint_text("e.g. C:\\Users\\you\\SyncedFolder"),
                            )
                            .changed()
                        {
                            s.settings.save();
                        }
                    });

                    ui.add_space(10.0);
                    if primary_button_fill(ui, !s.busy(), "Scan for duplicates").clicked()
                    {
                        s.start_duplicates_scan();
                    }
                });
            },
            |s, ui| {
                let busy = s.busy();
                card(ui, "Results", |ui| {
                    ui.horizontal_wrapped(|ui| {
                        stat_chip(
                            ui,
                            format!("{}", s.duplicates.groups.len()),
                            "groups",
                        );
                        stat_chip(
                            ui,
                            helpers::human_size(s.duplicates.total_wasted),
                            "reclaimable",
                        );
                        stat_chip(
                            ui,
                            format!("{}", s.duplicates.selected_files.len()),
                            "marked",
                        );
                    });
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Filter:").weak().small());
                        ui.add(
                            egui::TextEdit::singleline(&mut s.duplicates.filter)
                                .hint_text("type to filter results")
                                .desired_width(180.0),
                        );
                        if !s.duplicates.filter.is_empty()
                            && ui.small_button("Clear").clicked()
                        {
                            s.duplicates.filter.clear();
                        }
                        ui.separator();
                        ui.label(egui::RichText::new("Sort:").weak().small());
                        egui::ComboBox::from_id_source("dup_sort")
                            .selected_text(s.duplicates.sort.label())
                            .show_ui(ui, |ui| {
                                for sm in SortMode::all() {
                                    ui.selectable_value(
                                        &mut s.duplicates.sort,
                                        *sm,
                                        sm.label(),
                                    );
                                }
                            });
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(
                                        "First file in each group is kept",
                                    )
                                    .weak()
                                    .small(),
                                );
                            },
                        );
                    });
                    ui.add_space(4.0);

                    let filter = s.duplicates.filter.to_lowercase();
                    let sort = s.duplicates.sort;
                    let groups = &s.duplicates.groups;
                    let selected = &mut s.duplicates.selected_files;
                    let mut changed = false;
                    let list_h = Self::list_area_h(ui);
                    ui.allocate_ui(egui::vec2(ui.available_width(), list_h), |ui| {
                        egui::ScrollArea::vertical()
                            .id_source("duplicates_scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if groups.is_empty() {
                                    empty_state(
                                        ui,
                                        "No duplicate groups found yet — run a scan.",
                                    );
                                    return;
                                }
                                let mut view: Vec<(&DuplicateGroup, String)> = groups
                                    .iter()
                                    .filter(|g| {
                                        filter.is_empty()
                                            || g.files.iter().any(|f| {
                                                f.to_string_lossy()
                                                    .to_lowercase()
                                                    .contains(&filter)
                                            })
                                    })
                                    .map(|g| {
                                        (
                                            g,
                                            g.files
                                                .first()
                                                .map(|f| name_key(f))
                                                .unwrap_or_default(),
                                        )
                                    })
                                    .collect();
                                view.sort_by(|a, b| {
                                    // "Size" for a group = total reclaimable bytes.
                                    let wasted = |g: &DuplicateGroup| {
                                        g.size
                                            * (g.files.len().saturating_sub(1)) as u64
                                    };
                                    sort.compare(
                                        (wasted(a.0), a.1.as_str()),
                                        (wasted(b.0), b.1.as_str()),
                                    )
                                });
                                for (group, _) in view {
                                    ui.collapsing(
                                        format!(
                                            "{} files · {} each · {}",
                                            group.files.len(),
                                            helpers::human_size(group.size),
                                            &group.hash[..8.min(group.hash.len())]
                                        ),
                                        |ui| {
                                            for (i, file) in
                                                group.files.iter().enumerate()
                                            {
                                                ui.horizontal(|ui| {
                                                    if i == 0 {
                                                        ui.colored_label(
                                                            accent(ui), "keep",
                                                        );
                                                    } else {
                                                        let mut on =
                                                            selected.contains(file);
                                                        if ui
                                                            .checkbox(&mut on, "")
                                                            .changed()
                                                        {
                                                            changed = true;
                                                            if on {
                                                                selected
                                                                    .push(file.clone());
                                                            } else {
                                                                selected.retain(
                                                                    |x| x != file,
                                                                );
                                                            }
                                                        }
                                                    }
                                                    let resp = ui.monospace(
                                                        file.display().to_string(),
                                                    );
                                                    file_row_menu(&resp, file);
                                                });
                                            }
                                        },
                                    );
                                }
                            });
                    });

                    if changed {
                        s.duplicates.total_wasted = s.dup_wasted();
                    }

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !s.duplicates.groups.is_empty(),
                                egui::Button::new("Export report"),
                            )
                            .clicked()
                        {
                            let files: Vec<MatchedFile> = s
                                .duplicates
                                .groups
                                .iter()
                                .flat_map(|g| {
                                    g.files.iter().map(|p| MatchedFile {
                                        path: p.clone(),
                                        size: g.size,
                                    })
                                })
                                .collect();
                            match helpers::export_report(&files, "duplicates") {
                                Ok(p) => {
                                    s.status =
                                        format!("Report saved: {}", p.display());
                                    s.status_toast = 60;
                                }
                                Err(e) => {
                                    s.status = format!("Export error: {}", e);
                                    s.status_toast = 60;
                                }
                            }
                        }
                        ui.separator();
                        if ui
                            .add_enabled(
                                !busy && !s.duplicates.groups.is_empty(),
                                egui::Button::new("Select all").small(),
                            )
                            .clicked()
                        {
                            s.duplicates.selected_files = s
                                .duplicates
                                .groups
                                .iter()
                                .flat_map(|g| g.files.iter().skip(1).cloned())
                                .collect();
                            s.duplicates.total_wasted = s.dup_wasted();
                        }
                        if ui
                            .add_enabled(
                                !busy && !s.duplicates.groups.is_empty(),
                                egui::Button::new("Keep newest").small(),
                            )
                            .on_hover_text(
                                "Keep only the most recently modified copy in each group",
                            )
                            .clicked()
                        {
                            s.duplicates.selected_files =
                                dupes_select_keeping(&mut s.duplicates.groups, true);
                            s.duplicates.total_wasted = s.dup_wasted();
                        }
                        if ui
                            .add_enabled(
                                !busy && !s.duplicates.groups.is_empty(),
                                egui::Button::new("Keep oldest").small(),
                            )
                            .on_hover_text("Keep only the oldest copy in each group")
                            .clicked()
                        {
                            s.duplicates.selected_files =
                                dupes_select_keeping(&mut s.duplicates.groups, false);
                            s.duplicates.total_wasted = s.dup_wasted();
                        }
                        if ui
                            .add_enabled(
                                !busy && !s.duplicates.selected_files.is_empty(),
                                egui::Button::new("Clear").small(),
                            )
                            .clicked()
                        {
                            s.duplicates.selected_files.clear();
                            s.duplicates.total_wasted = 0;
                        }
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                let n = s.duplicates.selected_files.len();
                                if danger_button(
                                    ui,
                                    !busy && n > 0,
                                    format!("Clean {} marked", n),
                                )
                                .clicked()
                                {
                                    if s.settings.confirm_clean {
                                        s.confirm_title =
                                            "Clean duplicates?".to_string();
                                        s.confirm_body = format!(
                                            "Delete {} selected duplicate files ({})? The first file in each group is kept.",
                                            n,
                                            helpers::human_size(
                                                s.duplicates.total_wasted
                                            )
                                        );
                                        s.confirm_action =
                                            Some(ConfirmAction::CleanDuplicates);
                                    } else {
                                        s.start_clean_duplicates();
                                    }
                                }
                            },
                        );
                    });
                });
            },
        );
    }

    fn draw_large_files(&mut self, ui: &mut egui::Ui) {
        self.scan_page(
            ui,
            "large",
            |s, ui| {
                card(ui, "Scan setup", |ui| {
                    dir_picker(ui, &mut s.large_files.dir_path);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label("Larger than");
                        ui.add(
                            egui::Slider::new(&mut s.large_files.threshold_mb, 1..=10240)
                                .text("MB"),
                        );
                    });
                    ui.label(
                        egui::RichText::new(
                            "Finds the biggest files under the folder — the space hogs.",
                        )
                        .weak()
                        .small(),
                    );

                    ui.add_space(10.0);
                    if primary_button_fill(ui, !s.busy(), "Scan for large files")
                        .clicked()
                    {
                        s.start_large_files_scan();
                    }
                });
            },
            |s, ui| {
                let busy = s.busy();
                card(ui, "Results", |ui| {
                    ui.horizontal_wrapped(|ui| {
                        stat_chip(
                            ui,
                            format!("{}", s.large_files.files.len()),
                            "files",
                        );
                        stat_chip(
                            ui,
                            helpers::human_size(s.large_files.total_size),
                            "total",
                        );
                        stat_chip(
                            ui,
                            format!("{}", s.large_files.selected.len()),
                            "selected",
                        );
                    });
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Filter:").weak().small());
                        ui.add(
                            egui::TextEdit::singleline(&mut s.large_files.filter)
                                .hint_text("type to filter results")
                                .desired_width(180.0),
                        );
                        if !s.large_files.filter.is_empty()
                            && ui.small_button("Clear").clicked()
                        {
                            s.large_files.filter.clear();
                        }
                        ui.separator();
                        ui.label(egui::RichText::new("Sort:").weak().small());
                        egui::ComboBox::from_id_source("large_sort")
                            .selected_text(s.large_files.sort.label())
                            .show_ui(ui, |ui| {
                                for sm in SortMode::all() {
                                    ui.selectable_value(
                                        &mut s.large_files.sort,
                                        *sm,
                                        sm.label(),
                                    );
                                }
                            });
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .add_enabled(
                                        !busy && !s.large_files.selected.is_empty(),
                                        egui::Button::new("Clear sel.").small(),
                                    )
                                    .clicked()
                                {
                                    s.large_files.selected.clear();
                                }
                                if ui
                                    .add_enabled(
                                        !busy && !s.large_files.files.is_empty(),
                                        egui::Button::new("Select all").small(),
                                    )
                                    .on_hover_text(
                                        "Select all results — respects the active filter",
                                    )
                                    .clicked()
                                {
                                    let flt = s.large_files.filter.to_lowercase();
                                    s.large_files.selected = s
                                        .large_files
                                        .files
                                        .iter()
                                        .filter(|f| {
                                            flt.is_empty()
                                                || f.path
                                                    .to_string_lossy()
                                                    .to_lowercase()
                                                    .contains(&flt)
                                        })
                                        .map(|f| f.path.clone())
                                        .collect();
                                }
                            },
                        );
                    });
                    ui.add_space(4.0);

                    let filter = s.large_files.filter.to_lowercase();
                    let sort = s.large_files.sort;
                    let files = &s.large_files.files;
                    let selected = &mut s.large_files.selected;
                    let list_h = Self::list_area_h(ui);
                    if files.is_empty() {
                        ui.allocate_ui(
                            egui::vec2(ui.available_width(), list_h),
                            |ui| {
                                egui::ScrollArea::vertical()
                                    .id_source("large_files_scroll")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        empty_state(
                                            ui,
                                            "No large files found yet — run a scan.",
                                        )
                                    });
                            },
                        );
                    } else {
                        let mut view: Vec<(&LargeFile, String)> = files
                            .iter()
                            .filter(|f| {
                                filter.is_empty()
                                    || f.path
                                        .to_string_lossy()
                                        .to_lowercase()
                                        .contains(&filter)
                            })
                            .map(|f| (f, name_key(&f.path)))
                            .collect();
                        view.sort_by(|a, b| {
                            sort.compare((a.0.size, a.1.as_str()), (b.0.size, b.1.as_str()))
                        });
                        let row_h =
                            ui.text_style_height(&egui::TextStyle::Monospace) + 8.0;
                        ui.allocate_ui(
                            egui::vec2(ui.available_width(), list_h),
                            |ui| {
                                egui::ScrollArea::vertical()
                                    .id_source("large_files_scroll")
                                    .auto_shrink([false, false])
                                    .show_rows(ui, row_h, view.len(), |ui, range| {
                                        for (f, _) in &view[range] {
                                            ui.horizontal(|ui| {
                                                let mut on = selected.contains(&f.path);
                                                if ui.checkbox(&mut on, "").changed() {
                                                    if on {
                                                        selected.push(f.path.clone());
                                                    } else {
                                                        selected.retain(|x| x != &f.path);
                                                    }
                                                }
                                                let resp = ui.monospace(
                                                    f.path.display().to_string(),
                                                );
                                                file_row_menu(&resp, &f.path);
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        ui.label(
                                                            egui::RichText::new(
                                                                helpers::human_size(
                                                                    f.size,
                                                                ),
                                                            )
                                                            .weak(),
                                                        );
                                                        if ui
                                                            .small_button("Locate")
                                                            .clicked()
                                                        {
                                                            // Select the file itself,
                                                            // not just its parent.
                                                            reveal_in_explorer(&f.path);
                                                        }
                                                    },
                                                );
                                            });
                                        }
                                    });
                            },
                        );
                    }

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !s.large_files.files.is_empty(),
                                egui::Button::new("Export report"),
                            )
                            .clicked()
                        {
                            let files: Vec<MatchedFile> = s
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
                                    s.status =
                                        format!("Report saved: {}", p.display());
                                    s.status_toast = 60;
                                }
                                Err(e) => {
                                    s.status = format!("Export error: {}", e);
                                    s.status_toast = 60;
                                }
                            }
                        }
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                let n = s.large_files.selected.len();
                                if danger_button(
                                    ui,
                                    !busy && n > 0,
                                    format!("Clean {} selected", n),
                                )
                                .clicked()
                                {
                                    if s.settings.confirm_clean {
                                        s.confirm_title =
                                            "Delete selected large files?"
                                                .to_string();
                                        s.confirm_body =
                                            format!("Delete the {} selected files?", n);
                                        s.confirm_action =
                                            Some(ConfirmAction::CleanFiles(s.tab));
                                    } else {
                                        s.start_clean_selected(s.tab);
                                    }
                                }
                            },
                        );
                    });
                });
            },
        );
    }

    fn draw_system_cleaner(&mut self, ui: &mut egui::Ui) {
        self.scan_page(
            ui,
            "system",
            |s, ui| {
                card(ui, "Cleaning targets", |ui| {
                    let mut remove_idx: Option<usize> = None;
                    let mut toggled: Vec<(String, bool)> = Vec::new();
                    for (i, target) in s.system.targets.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut target.enabled, "").changed()
                                && !target.custom
                            {
                                toggled.push((target.name.clone(), target.enabled));
                            }
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(&target.name).strong(),
                                    );
                                    if target.custom {
                                        ui.label(
                                            egui::RichText::new("custom")
                                                .weak()
                                                .small(),
                                        );
                                        if ui.small_button("✕").clicked() {
                                            remove_idx = Some(i);
                                        }
                                    }
                                });
                                ui.label(
                                    egui::RichText::new(&target.description)
                                        .weak()
                                        .small(),
                                );
                                ui.label(
                                    egui::RichText::new(
                                        target.path.display().to_string(),
                                    )
                                    .monospace()
                                    .small()
                                    .weak(),
                                );
                            });
                        });
                        ui.separator();
                    }
                    if let Some(i) = remove_idx {
                        let removed = s.system.targets.remove(i);
                        // Compare as paths, not display strings — PathBuf
                        // normalizes things like a trailing backslash that the
                        // raw String keeps.
                        s.settings
                            .custom_targets
                            .retain(|c| PathBuf::from(&c.path) != removed.path);
                        s.settings.save();
                        s.add_log(&format!("Removed custom target: {}", removed.name));
                    }
                    for (name, enabled) in toggled {
                        if enabled {
                            s.settings.disabled_targets.retain(|n| n != &name);
                        } else if !s.settings.disabled_targets.contains(&name) {
                            s.settings.disabled_targets.push(name);
                        }
                        s.settings.save();
                    }

                    ui.collapsing("Add a custom target", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Name");
                            ui.text_edit_singleline(&mut s.new_target_name);
                        });
                        dir_picker(ui, &mut s.new_target_path);
                        let ok = !s.new_target_name.trim().is_empty()
                            && !s.new_target_path.trim().is_empty();
                        if ui
                            .add_enabled(ok, egui::Button::new("Add target"))
                            .clicked()
                        {
                            s.add_custom_target();
                        }
                    });

                    ui.add_space(10.0);
                    if primary_button_fill(ui, !s.busy(), "Scan system").clicked() {
                        s.start_system_scan();
                    }
                });
            },
            |s, ui| {
                let busy = s.busy();
                card(ui, "Results", |ui| {
                    ui.horizontal_wrapped(|ui| {
                        stat_chip(
                            ui,
                            format!("{}", s.system.matched_files.len()),
                            "files matched",
                        );
                        stat_chip(
                            ui,
                            helpers::human_size(s.system.total_matched_size),
                            "to clean",
                        );
                        stat_chip(
                            ui,
                            format!(
                                "{}",
                                s.system.targets.iter().filter(|t| t.enabled).count()
                            ),
                            "targets on",
                        );
                    });
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Filter:").weak().small());
                        ui.add(
                            egui::TextEdit::singleline(&mut s.system.filter)
                                .hint_text("type to filter results")
                                .desired_width(180.0),
                        );
                        if !s.system.filter.is_empty() && ui.small_button("Clear").clicked()
                        {
                            s.system.filter.clear();
                        }
                        ui.separator();
                        ui.label(egui::RichText::new("Sort:").weak().small());
                        egui::ComboBox::from_id_source("system_sort")
                            .selected_text(s.system.sort.label())
                            .show_ui(ui, |ui| {
                                for sm in SortMode::all() {
                                    ui.selectable_value(&mut s.system.sort, *sm, sm.label());
                                }
                            });
                    });
                    ui.add_space(4.0);

                    let filter = s.system.filter.to_lowercase();
                    let sort = s.system.sort;
                    let mut view: Vec<(&MatchedFile, String)> = s
                        .system
                        .matched_files
                        .iter()
                        .filter(|f| {
                            filter.is_empty()
                                || f.path
                                    .to_string_lossy()
                                    .to_lowercase()
                                    .contains(&filter)
                        })
                        .map(|f| (f, name_key(&f.path)))
                        .collect();
                    view.sort_by(|a, b| {
                        sort.compare((a.0.size, a.1.as_str()), (b.0.size, b.1.as_str()))
                    });
                    let refs: Vec<&MatchedFile> = view.iter().map(|(f, _)| *f).collect();
                    let list_h = Self::list_area_h(ui);
                    ui.allocate_ui(egui::vec2(ui.available_width(), list_h), |ui| {
                        matched_file_rows(ui, &refs, "system_scroll");
                    });

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !s.system.matched_files.is_empty(),
                                egui::Button::new("Export report"),
                            )
                            .clicked()
                        {
                            match helpers::export_report(&s.system.matched_files, "system")
                            {
                                Ok(p) => {
                                    s.status =
                                        format!("Report saved: {}", p.display());
                                    s.status_toast = 60;
                                }
                                Err(e) => {
                                    s.status = format!("Export error: {}", e);
                                    s.status_toast = 60;
                                }
                            }
                        }
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                let n = s.system.matched_files.len();
                                if danger_button(
                                    ui,
                                    !busy && n > 0,
                                    format!("Clean {} files", n),
                                )
                                .clicked()
                                {
                                    if s.settings.confirm_clean {
                                        s.confirm_title =
                                            "Clean system files?".to_string();
                                        s.confirm_body = format!(
                                            "Delete the {} matched system files ({})?",
                                            n,
                                            helpers::human_size(
                                                s.system.total_matched_size
                                            )
                                        );
                                        s.confirm_action =
                                            Some(ConfirmAction::CleanFiles(s.tab));
                                    } else {
                                        s.start_clean_selected(s.tab);
                                    }
                                }
                            },
                        );
                    });
                });
            },
        );
    }

    fn draw_empty_folders(&mut self, ui: &mut egui::Ui) {
        self.scan_page(
            ui,
            "empty",
            |s, ui| {
                card(ui, "Scan setup", |ui| {
                    dir_picker(ui, &mut s.empty_folders.dir_path);
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(
                            "Cascade-aware: folders that become empty after their children are removed are included.",
                        )
                        .weak()
                        .small(),
                    );

                    ui.add_space(10.0);
                    if primary_button_fill(ui, !s.busy(), "Scan for empty folders")
                        .clicked()
                    {
                        s.start_empty_folders_scan();
                    }
                });
            },
            |s, ui| {
                let busy = s.busy();
                card(ui, "Results", |ui| {
                    ui.horizontal_wrapped(|ui| {
                        stat_chip(
                            ui,
                            format!("{}", s.empty_folders.folders.len()),
                            "empty folders",
                        );
                    });
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Filter:").weak().small());
                        ui.add(
                            egui::TextEdit::singleline(&mut s.empty_folders.filter)
                                .hint_text("type to filter results")
                                .desired_width(180.0),
                        );
                        if !s.empty_folders.filter.is_empty()
                            && ui.small_button("Clear").clicked()
                        {
                            s.empty_folders.filter.clear();
                        }
                    });
                    ui.add_space(4.0);

                    let flt = s.empty_folders.filter.to_lowercase();
                    let list_h = Self::list_area_h(ui);
                    ui.allocate_ui(egui::vec2(ui.available_width(), list_h), |ui| {
                        if s.empty_folders.folders.is_empty() {
                            egui::ScrollArea::vertical()
                                .id_source("empty_folders_scroll")
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    empty_state(
                                        ui,
                                        "No empty folders found yet — run a scan.",
                                    )
                                });
                            return;
                        }
                        let view: Vec<&PathBuf> = s
                            .empty_folders
                            .folders
                            .iter()
                            .filter(|f| {
                                flt.is_empty()
                                    || f.to_string_lossy()
                                        .to_lowercase()
                                        .contains(&flt)
                            })
                            .collect();
                        let row_h =
                            ui.text_style_height(&egui::TextStyle::Monospace) + 8.0;
                        egui::ScrollArea::vertical()
                            .id_source("empty_folders_scroll")
                            .auto_shrink([false, false])
                            .show_rows(ui, row_h, view.len(), |ui, range| {
                                for f in &view[range] {
                                    let resp =
                                        ui.monospace(f.display().to_string());
                                    file_row_menu(&resp, f);
                                }
                            });
                    });

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !s.empty_folders.folders.is_empty(),
                                egui::Button::new("Export report"),
                            )
                            .clicked()
                        {
                            match helpers::export_path_report(
                                &s.empty_folders.folders,
                                "empty_folders",
                            ) {
                                Ok(p) => {
                                    s.status =
                                        format!("Report saved: {}", p.display());
                                    s.status_toast = 60;
                                }
                                Err(e) => {
                                    s.status = format!("Export error: {}", e);
                                    s.status_toast = 60;
                                }
                            }
                        }
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                let n = s.empty_folders.folders.len();
                                if danger_button(
                                    ui,
                                    !busy && n > 0,
                                    format!("Remove {} folders", n),
                                )
                                .clicked()
                                {
                                    if s.settings.confirm_clean {
                                        s.confirm_title =
                                            "Remove empty folders?".to_string();
                                        s.confirm_body = format!(
                                            "Remove {} empty folders (including cascades)?",
                                            n
                                        );
                                        s.confirm_action =
                                            Some(ConfirmAction::CleanEmptyFolders);
                                    } else {
                                        s.start_clean_empty_folders();
                                    }
                                }
                            },
                        );
                    });
                });
            },
        );
    }

    fn draw_folder_sizes(&mut self, ui: &mut egui::Ui) {
        self.scan_page(
            ui,
            "sizes",
            |s, ui| {
                card(ui, "Scan setup", |ui| {
                    dir_picker(ui, &mut s.folder_sizes.dir_path);
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(
                            "Totals up each top-level subfolder so you can see what's taking the space.",
                        )
                        .weak()
                        .small(),
                    );

                    ui.add_space(10.0);
                    if primary_button_fill(ui, !s.busy(), "Analyze").clicked() {
                        s.start_folder_sizes_scan();
                    }
                });
            },
            |s, ui| {
                card(ui, "Results", |ui| {
                    if s.folder_sizes.entries.is_empty() {
                        let list_h = Self::list_area_h(ui);
                        ui.allocate_ui(
                            egui::vec2(ui.available_width(), list_h),
                            |ui| {
                                egui::ScrollArea::vertical()
                                    .id_source("folder_sizes_scroll")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        empty_state(
                                            ui,
                                            "No breakdown yet — run an analysis.",
                                        )
                                    });
                            },
                        );
                        return;
                    }
                    ui.horizontal_wrapped(|ui| {
                        stat_chip(
                            ui,
                            helpers::human_size(s.folder_sizes.total),
                            "total",
                        );
                        stat_chip(
                            ui,
                            format!("{}", s.folder_sizes.entries.len()),
                            "top-level entries",
                        );
                    });
                    ui.add_space(6.0);
                    let max = s
                        .folder_sizes
                        .entries
                        .first()
                        .map(|e| e.size)
                        .unwrap_or(1)
                        .max(1);
                    let total = s.folder_sizes.total.max(1);
                    let list_h = Self::list_area_h(ui);
                    ui.allocate_ui(egui::vec2(ui.available_width(), list_h), |ui| {
                        egui::ScrollArea::vertical()
                            .id_source("folder_sizes_scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for e in &s.folder_sizes.entries {
                                    let share = e.size as f32 / max as f32;
                                    let pct = e.size as f64 / total as f64 * 100.0;
                                    let resp = ui.add(
                                        egui::ProgressBar::new(share)
                                            .fill(accent(ui))
                                            .text(format!(
                                                "{} — {} ({:.0}%)",
                                                e.name,
                                                helpers::human_size(e.size),
                                                pct
                                            )),
                                    );
                                    // Double-click / right-click to reveal the folder.
                                    resp.clone()
                                        .on_hover_text(
                                            "Double-click to open in Explorer",
                                        )
                                        .context_menu(|ui| {
                                            if ui
                                                .button("Reveal in Explorer")
                                                .clicked()
                                            {
                                                reveal_in_explorer(&e.path);
                                                ui.close_menu();
                                            }
                                            if ui.button("Copy path").clicked() {
                                                ui.ctx().output_mut(|o| {
                                                    o.copied_text =
                                                        e.path.display().to_string();
                                                });
                                                ui.close_menu();
                                            }
                                        });
                                    if resp.double_clicked() {
                                        reveal_in_explorer(&e.path);
                                    }
                                    ui.add_space(2.0);
                                }
                            });
                    });
                });
            },
        );
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

        // Summary chips — total capacity and free space across all drives.
        let total: u64 = self.storage_disks.iter().map(|d| d.total).sum();
        let free: u64 = self.storage_disks.iter().map(|d| d.available).sum();
        ui.horizontal_wrapped(|ui| {
            stat_chip(ui, format!("{}", self.storage_disks.len()), "drives");
            stat_chip(ui, helpers::human_size(total), "total");
            stat_chip(ui, helpers::human_size(free), "free");
        });
        ui.add_space(8.0);

        let wide = ui.available_width() > 760.0;
        let draw_disk = |disk: &DiskEntry, ui: &mut egui::Ui| {
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
        };

        if wide {
            for pair in self.storage_disks.chunks(2) {
                ui.columns(2, |cols| {
                    for (i, disk) in pair.iter().enumerate() {
                        draw_disk(disk, &mut cols[i]);
                    }
                });
                ui.add_space(8.0);
            }
        } else {
            for disk in &self.storage_disks {
                draw_disk(disk, ui);
                ui.add_space(8.0);
            }
        }

        ui.label(
            egui::RichText::new("Read-only view — nothing here deletes anything.")
                .weak()
                .small(),
        );
    }

    fn draw_changelog(&mut self, ui: &mut egui::Ui) {
        let releases = parse_releases(CHANGELOG);
        if releases.is_empty() {
            empty_state(ui, "No release notes bundled.");
            return;
        }
        let sel = self
            .changelog_version
            .clone()
            .unwrap_or_else(|| releases[0].version.clone());

        card(ui, "What's new", |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Version:").weak().small());
                egui::ComboBox::from_id_source("changelog_version")
                    .selected_text(format!("v{}", sel))
                    .show_ui(ui, |ui| {
                        for r in &releases {
                            if ui
                                .selectable_label(
                                    r.version == sel,
                                    format!("v{}", r.version),
                                )
                                .clicked()
                            {
                                self.changelog_version = Some(r.version.clone());
                            }
                        }
                    });
                if sel == releases[0].version {
                    ui.label(
                        egui::RichText::new("latest").small().color(accent(ui)),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("View on GitHub").clicked() {
                        let _ = open::that(format!(
                            "{}/releases",
                            self.settings.github_url.trim_end_matches('/')
                        ));
                    }
                });
            });
            ui.add_space(8.0);

            let Some(rel) = releases
                .iter()
                .find(|r| r.version == sel)
                .or_else(|| releases.first())
            else {
                empty_state(ui, "No notes for this version.");
                return;
            };

            egui::ScrollArea::vertical()
                .id_source("changelog_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("v{}", rel.version))
                                .size(18.0)
                                .strong(),
                        );
                        ui.label(egui::RichText::new(&rel.date).weak());
                    });
                    if !rel.intro.is_empty() {
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new(&rel.intro).weak().small());
                    }
                    ui.add_space(8.0);
                    for sec in &rel.sections {
                        ui.colored_label(
                            changelog_section_color(&sec.title),
                            egui::RichText::new(&sec.title).strong(),
                        );
                        ui.add_space(2.0);
                        for item in &sec.items {
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("•").weak());
                                ui.add(egui::Label::new(item).wrap(true));
                            });
                        }
                        ui.add_space(6.0);
                    }
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
        // Load the app icon once for the hero card.
        if self.icon_tex.is_none() {
            let icon = themes::generate_icon();
            self.icon_tex = Some(ui.ctx().load_texture(
                "about_icon",
                egui::ColorImage::from_rgba_unmultiplied(
                    [icon.width as usize, icon.height as usize],
                    &icon.rgba,
                ),
                egui::TextureOptions::LINEAR,
            ));
        }

        // Hero — icon, name, version, tagline, quick links.
        card(ui, "", |ui| {
            ui.horizontal(|ui| {
                if let Some(tex) = &self.icon_tex {
                    ui.image((tex.id(), egui::vec2(64.0, 64.0)));
                    ui.add_space(12.0);
                }
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Cleaner").size(24.0).strong());
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "v{}",
                                env!("CARGO_PKG_VERSION")
                            ))
                            .color(accent(ui)),
                        );
                        ui.label(
                            egui::RichText::new("· Rust + eframe/egui")
                                .weak()
                                .small(),
                        );
                    });
                    ui.label(
                        egui::RichText::new(
                            "A fast, safe system-cleaning utility — \
                             no telemetry, no accounts, no network calls.",
                        )
                        .weak()
                        .small(),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.vertical(|ui| {
                        if ui.button("Release notes").clicked() {
                            self.tab = Tab::Changelog;
                        }
                        if ui.button("GitHub").clicked() {
                            let _ = open::that(&self.settings.github_url);
                        }
                    });
                });
            });
        });

        ui.add_space(10.0);

        card(ui, "Highlights", |ui| {
            let feats = [
                "Custom cleaning with filters",
                "Duplicate finder (two-stage hashing)",
                "Large file finder",
                "System junk cleaner",
                "Empty folder cleaner",
                "Secure Delete (3-pass shredder)",
                "6 themes + custom accent colors",
                "System tray + scheduled scans",
                "Recycle-bin deletes & dry run",
                "Storage overview & reports",
            ];
            ui.columns(2, |cols| {
                for (i, f) in feats.iter().enumerate() {
                    cols[i % 2].label(format!("• {}", f));
                }
            });
        });

        ui.add_space(10.0);

        card(ui, "Privacy", |ui| {
            ui.label(
                "Cleaner never sends anything anywhere — no telemetry, no analytics, \
                 no accounts, no network calls. Everything it writes stays on this PC.",
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "Your data folder: {}",
                    settings::data_dir().display()
                ))
                .weak()
                .small()
                .monospace(),
            );
            ui.label(
                egui::RichText::new(
                    "It contains settings, the history log, and exported reports — \
                     plus a README.txt explaining each file. Delete the folder to \
                     reset the app completely.",
                )
                .weak()
                .small(),
            );
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
                if ui.button("Open data folder").clicked() {
                    reveal_in_explorer(&settings::data_dir());
                }
                if ui.button("Reveal settings file").clicked() {
                    // Make sure it exists on disk before Explorer opens it.
                    self.settings.save();
                    reveal_in_explorer(&settings::settings_file_path());
                }
                if ui.button("Open history log").clicked() {
                    reveal_in_explorer(&self.settings.log_path());
                }
            });
        });

        ui.add_space(10.0);

        card(ui, "Report a bug", |ui| {
            ui.label(
                "Something broken or acting weird? Send a report with what you \
                 did and what happened — the button below opens your mail app \
                 with a template already filled in.",
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button("Email a bug report")
                    .on_hover_text("Opens your default mail app, addressed to the developer")
                    .clicked()
                {
                    let body = format!(
                        "Cleaner version: {}\nWindows: {}\n\nWhat I did:\n\n\nWhat happened:\n\n\nWhat I expected:\n\n",
                        env!("CARGO_PKG_VERSION"),
                        std::env::consts::OS
                    );
                    let url = format!(
                        "mailto:emrebelgrad@gmail.com?subject=Cleaner%20bug%20report%20(v{})&body={}",
                        env!("CARGO_PKG_VERSION"),
                        url_encode(&body)
                    );
                    let _ = open::that(url);
                }
                if ui
                    .button("Open GitHub issues")
                    .on_hover_text("File an issue on the repository instead")
                    .clicked()
                {
                    let _ =
                        open::that("https://github.com/w0wzahh/cleaner/issues");
                }
                if ui
                    .button("Copy version info")
                    .on_hover_text("Copies version + data-folder path to the clipboard for the report")
                    .clicked()
                {
                    ui.output_mut(|o| {
                        o.copied_text = format!(
                            "Cleaner v{} | data folder: {}",
                            env!("CARGO_PKG_VERSION"),
                            settings::data_dir().display()
                        )
                    });
                }
            });
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "Contact: emrebelgrad@gmail.com — attaching {} helps a lot.",
                    self.settings.log_path().display()
                ))
                .weak()
                .small(),
            );
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
            let v = self.effective_visuals();
            ctx.set_visuals(v.clone());
            self.theme_anim.to = v;
            ctx.set_zoom_factor(self.settings.ui_zoom.clamp(0.7, 1.6));
            self.initial_theme_applied = true;
        }
        if !self.tray_setup_done {
            self.setup_tray(ctx);
        }
        // "Start in tray" / --minimized: park once the tray icon exists.
        // If the tray couldn't be created, show the window instead — never
        // strand the user with an invisible app.
        if self.start_in_tray && self.tray_setup_done {
            self.start_in_tray = false;
            if self.tray.is_some() {
                self.hide_to_tray(ctx);
            } else {
                helpers::show_main_window();
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                self.add_log("Tray icon unavailable — showing the window.");
            }
        }

        // F5 rescan the current tab.
        if !self.busy()
            && self.confirm_action.is_none()
            && ctx.input(|i| i.key_pressed(egui::Key::F5))
        {
            match self.tab {
                Tab::CustomClean => self.start_custom_scan(),
                Tab::Duplicates => self.start_duplicates_scan(),
                Tab::LargeFiles => self.start_large_files_scan(),
                Tab::SystemCleaner => self.start_system_scan(),
                Tab::EmptyFolders => self.start_empty_folders_scan(),
                Tab::FolderSizes => self.start_folder_sizes_scan(),
                _ => {}
            }
        }

        self.update_theme_animation(ctx);
        self.poll_messages(ctx);
        self.maybe_run_scheduled();
        self.flush_log();
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

        // Reflect busy status in the window title and tray tooltip — only
        // pushed when it changes.
        let title = if self.busy() {
            format!("Cleaner — {}", self.status)
        } else {
            "Cleaner".to_string()
        };
        if title != self.window_title {
            self.window_title = title.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
        let tip = self.window_title.clone();
        if tip != self.tray_tooltip {
            if let Some(tray) = &self.tray {
                let _ = tray.set_tooltip(Some(tip.as_str()));
            }
            self.tray_tooltip = tip;
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
                    if self.tray.is_some()
                        && ui
                            .button("To tray")
                            .on_hover_text(
                                "Hide the window — Cleaner keeps running in the system tray. Double-click the tray icon to bring it back.",
                            )
                            .clicked()
                    {
                        self.hide_to_tray(ctx);
                    }
                    // right_to_left order: this renders right after the combo,
                    // giving "Theme: [picker] [Customize]".
                    if ui
                        .button("Customize")
                        .on_hover_text("Accent colors and UI scale")
                        .clicked()
                    {
                        self.customize_open = true;
                    }
                    let mut new_theme: Option<themes::Theme> = None;
                    // ComboBox drifts vertically inside a right_to_left row
                    // (egui #7412/#4165) — give it its own centered child UI
                    // so it stays level with the buttons.
                    ui.allocate_ui_with_layout(
                        egui::vec2(120.0, 28.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
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
                        },
                    );
                    // right_to_left: added after the combo so it renders to
                    // its left ("Theme: [Dark ▾]").
                    ui.label("Theme:");
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
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
            egui::Window::new(&self.confirm_title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label(&self.confirm_body);
                    ui.add_space(4.0);
                    if self.settings.dry_run {
                        ui.label(
                            egui::RichText::new(
                                "Dry run is enabled — nothing will actually be deleted.",
                            )
                            .weak()
                            .small(),
                        );
                    } else if self.settings.secure_delete {
                        ui.colored_label(
                            danger_color(),
                            "Secure delete is on — files are overwritten 3 times and can't be recovered.",
                        );
                    } else if self.settings.use_trash {
                        ui.label(
                            egui::RichText::new(
                                "Files go to the Recycle Bin — you can restore them from there.",
                            )
                            .weak()
                            .small(),
                        );
                    } else {
                        ui.colored_label(
                            warn_color(),
                            "Files will be permanently deleted — no Recycle Bin.",
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
            // Act before clearing: handle_confirm() takes confirm_action,
            // so clearing first would swallow the action entirely.
            if do_action {
                self.handle_confirm();
            }
            if close {
                self.confirm_action = None;
            }
        }

        // -- appearance customization window -----------------------------------
        if self.customize_open {
            let mut open = self.customize_open;
            let mut retint = false;
            let mut dirty = false;
            egui::Window::new("Customize appearance")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label("Accent — buttons, selections, and links.");
                    ui.horizontal(|ui| {
                        let mut c = self.settings.accent_rgb.unwrap_or_else(|| {
                            let a = self.settings.theme.accent();
                            [a.r(), a.g(), a.b()]
                        });
                        if egui::widgets::color_picker::color_edit_button_srgb(
                            ui, &mut c,
                        )
                        .changed()
                        {
                            self.settings.accent_rgb = Some(c);
                            retint = true;
                            dirty = true;
                        }
                        if self.settings.accent_rgb.is_some()
                            && ui.small_button("Theme default").clicked()
                        {
                            self.settings.accent_rgb = None;
                            retint = true;
                            dirty = true;
                        }
                    });
                    ui.add_space(8.0);
                    ui.label("Secondary accent — gradients and the dashboard ring.");
                    ui.horizontal(|ui| {
                        let mut c2 = self.settings.accent2_rgb.unwrap_or_else(|| {
                            let a = self.accent2();
                            [a.r(), a.g(), a.b()]
                        });
                        if egui::widgets::color_picker::color_edit_button_srgb(
                            ui, &mut c2,
                        )
                        .changed()
                        {
                            self.settings.accent2_rgb = Some(c2);
                            dirty = true;
                        }
                        if self.settings.accent2_rgb.is_some()
                            && ui.small_button("Theme default").clicked()
                        {
                            self.settings.accent2_rgb = None;
                            dirty = true;
                        }
                    });
                    ui.add_space(8.0);
                    ui.label("Interface scale");
                    if ui
                        .add(
                            egui::Slider::new(&mut self.settings.ui_zoom, 0.75..=1.5)
                                .text("zoom"),
                        )
                        .changed()
                    {
                        ctx.set_zoom_factor(self.settings.ui_zoom);
                        dirty = true;
                    }
                    if (self.settings.ui_zoom - 1.0).abs() > f32::EPSILON
                        && ui.small_button("Reset to 100%").clicked()
                    {
                        self.settings.ui_zoom = 1.0;
                        ctx.set_zoom_factor(1.0);
                        dirty = true;
                    }
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Changes apply instantly and save automatically.")
                            .weak()
                            .small(),
                    );
                });
            self.customize_open = open;
            if retint {
                // Re-tint live visuals; cancel any in-flight theme
                // animation so the override isn't overwritten mid-lerp.
                let v = self.effective_visuals();
                self.theme_anim.to = v.clone();
                self.theme_anim.active = false;
                ctx.set_visuals(v);
            }
            if dirty {
                self.settings.save();
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Stop any in-flight worker so a clean can't be cut mid-file.
        self.cancel_flag.store(true, Ordering::Relaxed);
        // Make sure the tray icon disappears instead of lingering.
        self.tray.take();
        // Persist anything still in the log buffer.
        self.flush_log();
    }
}

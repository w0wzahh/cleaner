//! Theme palette, animated transitions, and the procedurally-generated app icon.
//!
//! The five existing themes are preserved; the visuals have been tuned so the app
//! feels more cohesive and modern while still staying lightweight.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use eframe::egui;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Midnight,
    Dark,
    Light,
    Nord,
    Dracula,
    Solarized,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Midnight
    }
}

impl Theme {
    pub fn label(&self) -> &'static str {
        match self {
            Theme::Midnight => "Midnight",
            Theme::Dark => "Dark",
            Theme::Light => "Light",
            Theme::Nord => "Nord",
            Theme::Dracula => "Dracula",
            Theme::Solarized => "Solarized",
        }
    }

    pub fn all() -> &'static [Theme] {
        &[
            Theme::Midnight,
            Theme::Dark,
            Theme::Light,
            Theme::Nord,
            Theme::Dracula,
            Theme::Solarized,
        ]
    }

    pub fn visuals(&self) -> egui::Visuals {
        match self {
            Theme::Midnight => midnight(),
            Theme::Dark => dark(),
            Theme::Light => light(),
            Theme::Nord => nord(),
            Theme::Dracula => dracula(),
            Theme::Solarized => solarized(),
        }
    }

    /// Solid accent color used for primary buttons, selections, and links.
    pub fn accent(&self) -> egui::Color32 {
        match self {
            Theme::Midnight => egui::Color32::from_rgb(0xFF, 0x3D, 0x9A),
            Theme::Dark => egui::Color32::from_rgb(0x63, 0x66, 0xF1),
            Theme::Light => egui::Color32::from_rgb(0x4F, 0x46, 0xE5),
            Theme::Nord => egui::Color32::from_rgb(0x5E, 0x81, 0xAC),
            Theme::Dracula => egui::Color32::from_rgb(0x9C, 0x6B, 0xE8),
            Theme::Solarized => egui::Color32::from_rgb(0x26, 0x8B, 0xD2),
        }
    }

    /// Secondary accent for gradients (ring, chart highlights).
    pub fn accent2(&self) -> egui::Color32 {
        match self {
            Theme::Midnight => egui::Color32::from_rgb(0x8B, 0x5C, 0xF6),
            _ => self.accent(),
        }
    }
}

// -----------------------------------------------------------------------------
// theme visuals
// -----------------------------------------------------------------------------

/// Shared polish: accent wiring plus rounded widgets/windows.
fn finish(mut v: egui::Visuals, accent: egui::Color32) -> egui::Visuals {
    v.hyperlink_color = accent;
    v.selection.bg_fill = accent;
    v.selection.stroke = egui::Stroke::new(1.0f32, egui::Color32::WHITE);
    let r = egui::Rounding::same(6.0);
    v.widgets.noninteractive.rounding = r;
    v.widgets.inactive.rounding = r;
    v.widgets.hovered.rounding = r;
    v.widgets.active.rounding = r;
    v.widgets.open.rounding = r;
    v.window_rounding = egui::Rounding::same(10.0);
    v
}

/// One-time global spacing tweaks (independent of the active theme).
pub fn apply_spacing(ctx: &egui::Context) {
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(14.0, 6.0);
        s.spacing.indent = 20.0;
    });
}

fn midnight() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = egui::Color32::from_rgb(0x16, 0x0F, 0x26);
    v.window_fill = egui::Color32::from_rgb(0x1E, 0x15, 0x33);
    v.extreme_bg_color = egui::Color32::from_rgb(0x0E, 0x08, 0x18);
    v.faint_bg_color = egui::Color32::from_rgb(0x25, 0x1A, 0x40);
    v.override_text_color = Some(egui::Color32::from_rgb(0xED, 0xEA, 0xF6));
    finish(v, Theme::Midnight.accent())
}

fn dark() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = egui::Color32::from_rgb(0x1B, 0x1D, 0x24);
    v.window_fill = egui::Color32::from_rgb(0x22, 0x25, 0x2E);
    v.extreme_bg_color = egui::Color32::from_rgb(0x12, 0x14, 0x19);
    v.faint_bg_color = egui::Color32::from_rgb(0x26, 0x2A, 0x35);
    v.override_text_color = Some(egui::Color32::from_rgb(0xE6, 0xE6, 0xEE));
    finish(v, Theme::Dark.accent())
}

fn light() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    v.panel_fill = egui::Color32::from_rgb(0xF4, 0xF4, 0xF7);
    v.window_fill = egui::Color32::from_rgb(0xFF, 0xFF, 0xFF);
    v.extreme_bg_color = egui::Color32::from_rgb(0xE9, 0xE9, 0xEE);
    v.faint_bg_color = egui::Color32::from_rgb(0xFF, 0xFF, 0xFF);
    v.override_text_color = Some(egui::Color32::from_rgb(0x20, 0x22, 0x28));
    finish(v, Theme::Light.accent())
}

fn nord() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = egui::Color32::from_rgb(0x24, 0x2B, 0x35);
    v.window_fill = egui::Color32::from_rgb(0x2F, 0x37, 0x46);
    v.extreme_bg_color = egui::Color32::from_rgb(0x1C, 0x23, 0x2E);
    v.faint_bg_color = egui::Color32::from_rgb(0x34, 0x3D, 0x4C);
    v.override_text_color = Some(egui::Color32::from_rgb(0xD8, 0xDE, 0xE9));
    finish(v, Theme::Nord.accent())
}

fn dracula() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = egui::Color32::from_rgb(0x23, 0x24, 0x30);
    v.window_fill = egui::Color32::from_rgb(0x28, 0x2A, 0x37);
    v.extreme_bg_color = egui::Color32::from_rgb(0x1A, 0x1B, 0x25);
    v.faint_bg_color = egui::Color32::from_rgb(0x31, 0x33, 0x42);
    v.override_text_color = Some(egui::Color32::from_rgb(0xF8, 0xF8, 0xF2));
    finish(v, Theme::Dracula.accent())
}

fn solarized() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = egui::Color32::from_rgb(0x00, 0x28, 0x34);
    v.window_fill = egui::Color32::from_rgb(0x07, 0x38, 0x46);
    v.extreme_bg_color = egui::Color32::from_rgb(0x00, 0x1F, 0x28);
    v.faint_bg_color = egui::Color32::from_rgb(0x0A, 0x3B, 0x48);
    v.override_text_color = Some(egui::Color32::from_rgb(0x93, 0xA1, 0xA1));
    finish(v, Theme::Solarized.accent())
}

// -----------------------------------------------------------------------------
// theme animation
// -----------------------------------------------------------------------------

pub struct ThemeAnim {
    pub from: egui::Visuals,
    pub to: egui::Visuals,
    pub start: f64,
    pub duration: f64,
    pub active: bool,
}

impl ThemeAnim {
    pub fn new(initial: egui::Visuals) -> Self {
        Self {
            from: initial.clone(),
            to: initial,
            start: 0.0,
            duration: 0.35,
            active: false,
        }
    }

    pub fn start(&mut self, from: egui::Visuals, to: egui::Visuals, now: f64) {
        self.from = from;
        self.to = to;
        self.start = now;
        self.active = true;
    }
}

pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

pub fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let r = a.r() as f32 + (b.r() as f32 - a.r() as f32) * t;
    let g = a.g() as f32 + (b.g() as f32 - a.g() as f32) * t;
    let bl = a.b() as f32 + (b.b() as f32 - a.b() as f32) * t;
    let al = a.a() as f32 + (b.a() as f32 - a.a() as f32) * t;
    egui::Color32::from_rgba_unmultiplied(
        r.clamp(0.0, 255.0) as u8,
        g.clamp(0.0, 255.0) as u8,
        bl.clamp(0.0, 255.0) as u8,
        al.clamp(0.0, 255.0) as u8,
    )
}

pub fn lerp_visuals(a: &egui::Visuals, b: &egui::Visuals, t: f32) -> egui::Visuals {
    let mut v = b.clone();
    v.panel_fill = lerp_color(a.panel_fill, b.panel_fill, t);
    v.window_fill = lerp_color(a.window_fill, b.window_fill, t);
    v.extreme_bg_color = lerp_color(a.extreme_bg_color, b.extreme_bg_color, t);
    v.faint_bg_color = lerp_color(a.faint_bg_color, b.faint_bg_color, t);
    v.override_text_color = match (a.override_text_color, b.override_text_color) {
        (Some(ca), Some(cb)) => Some(lerp_color(ca, cb, t)),
        (None, None) => None,
        (Some(ca), None) => Some(egui::Color32::from_rgba_unmultiplied(
            ca.r(),
            ca.g(),
            ca.b(),
            ((1.0 - t) * 255.0) as u8,
        )),
        (None, Some(cb)) => Some(egui::Color32::from_rgba_unmultiplied(
            cb.r(),
            cb.g(),
            cb.b(),
            (t * 255.0) as u8,
        )),
    };
    v
}

// -----------------------------------------------------------------------------
// procedurally generated app icon — a broom on a rounded gradient tile
// -----------------------------------------------------------------------------

/// Distance from point to segment (capsule SDF helper).
fn sd_segment(px: f32, py: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let pax = px - a.0;
    let pay = py - a.1;
    let bax = b.0 - a.0;
    let bay = b.1 - a.1;
    let h = ((pax * bax + pay * bay) / (bax * bax + bay * bay)).clamp(0.0, 1.0);
    let dx = pax - bax * h;
    let dy = pay - bay * h;
    (dx * dx + dy * dy).sqrt()
}

/// Signed distance to a centered rounded square (negative inside).
fn sd_round_box(px: f32, py: f32, half: f32, r: f32) -> f32 {
    let qx = (px - half).abs() - (half - r);
    let qy = (py - half).abs() - (half - r);
    let ax = qx.max(0.0);
    let ay = qy.max(0.0);
    (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - r
}

/// Signed distance to a triangle (positive inside).
fn tri_dist(px: f32, py: f32, a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    // Normalize winding so "inside" is always positive.
    let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let (b, c) = if area < 0.0 { (c, b) } else { (b, c) };
    let edge = |p: (f32, f32), q: (f32, f32)| {
        let ex = q.0 - p.0;
        let ey = q.1 - p.1;
        let len = (ex * ex + ey * ey).sqrt().max(1e-6);
        (ex * (py - p.1) - ey * (px - p.0)) / len
    };
    edge(a, b).min(edge(b, c)).min(edge(c, a))
}

fn coverage(d: f32) -> f32 {
    (d + 0.75) / 1.5
}

pub fn generate_icon() -> Arc<egui::IconData> {
    let size: u32 = 128;
    let s = size as f32;
    let half = s / 2.0;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    // Broom geometry — handle top-right, bristles fanning down-left.
    let handle_a = (92.0, 24.0);
    let handle_b = (56.0, 58.0);
    let ferrule_a = (50.0, 54.0);
    let ferrule_b = (62.0, 66.0);
    let apex = (56.0, 62.0);
    let base_a = (16.0, 94.0);
    let base_b = (62.0, 112.0);
    let base_mid = ((base_a.0 + base_b.0) / 2.0, (base_a.1 + base_b.1) / 2.0);
    let fan_dir = (base_mid.0 - apex.0, base_mid.1 - apex.1);
    let fan_len2 = fan_dir.0 * fan_dir.0 + fan_dir.1 * fan_dir.1;

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let idx = ((y * size + x) * 4) as usize;

            // Rounded-square tile, indigo gradient top-left -> bottom-right.
            let d_bg = sd_round_box(px, py, half, 26.0);
            let cov_bg = coverage(-d_bg).clamp(0.0, 1.0);
            if cov_bg <= 0.0 {
                continue;
            }
            let t = ((px + py) / (2.0 * s)).clamp(0.0, 1.0);
            let mut r = 99.0 + (56.0 - 99.0) * t;
            let mut g = 102.0 + (52.0 - 102.0) * t;
            let mut b = 241.0 + (160.0 - 241.0) * t;

            // Bristles: triangle with a light-to-dark straw gradient.
            let d_tri = tri_dist(px, py, apex, base_a, base_b);
            let cov_tri = coverage(d_tri).clamp(0.0, 1.0);
            if cov_tri > 0.0 {
                let proj = (((px - apex.0) * fan_dir.0 + (py - apex.1) * fan_dir.1)
                    / fan_len2)
                    .clamp(0.0, 1.0);
                let mut br = 242.0 + (212.0 - 242.0) * proj;
                let mut bg_ = 200.0 + (152.0 - 200.0) * proj;
                let mut bb = 110.0 + (66.0 - 110.0) * proj;

                // Thin darker strokes suggest individual bristle strands.
                for target in [(26.0, 98.0), (38.0, 103.0), (50.0, 108.0)] {
                    let d_line = sd_segment(px, py, apex, target);
                    let strand = ((1.4 - d_line) / 1.2).clamp(0.0, 1.0);
                    br *= 1.0 - 0.22 * strand;
                    bg_ *= 1.0 - 0.22 * strand;
                    bb *= 1.0 - 0.22 * strand;
                }

                r = r * (1.0 - cov_tri) + br * cov_tri;
                g = g * (1.0 - cov_tri) + bg_ * cov_tri;
                b = b * (1.0 - cov_tri) + bb * cov_tri;
            }

            // Ferrule (the band where the handle meets the bristles).
            let d_fe = sd_segment(px, py, ferrule_a, ferrule_b) - 5.5;
            let cov_fe = coverage(-d_fe).clamp(0.0, 1.0);
            if cov_fe > 0.0 {
                r = r * (1.0 - cov_fe) + 150.0 * cov_fe;
                g = g * (1.0 - cov_fe) + 152.0 * cov_fe;
                b = b * (1.0 - cov_fe) + 168.0 * cov_fe;
            }

            // Wooden handle.
            let d_ha = sd_segment(px, py, handle_a, handle_b) - 4.5;
            let cov_ha = coverage(-d_ha).clamp(0.0, 1.0);
            if cov_ha > 0.0 {
                r = r * (1.0 - cov_ha) + 158.0 * cov_ha;
                g = g * (1.0 - cov_ha) + 102.0 * cov_ha;
                b = b * (1.0 - cov_ha) + 60.0 * cov_ha;
            }

            rgba[idx] = r.clamp(0.0, 255.0) as u8;
            rgba[idx + 1] = g.clamp(0.0, 255.0) as u8;
            rgba[idx + 2] = b.clamp(0.0, 255.0) as u8;
            rgba[idx + 3] = (cov_bg * 255.0) as u8;
        }
    }

    Arc::new(egui::IconData {
        rgba,
        width: size,
        height: size,
    })
}

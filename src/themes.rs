//! Theme palette, animated transitions, and the procedurally-generated app icon.
//!
//! The five existing themes are preserved; the visuals have been tuned so the app
//! feels more cohesive and modern while still staying lightweight.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use eframe::egui;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
    Nord,
    Dracula,
    Solarized,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Dark
    }
}

impl Theme {
    pub fn label(&self) -> &'static str {
        match self {
            Theme::Dark => "Dark",
            Theme::Light => "Light",
            Theme::Nord => "Nord",
            Theme::Dracula => "Dracula",
            Theme::Solarized => "Solarized",
        }
    }

    pub fn all() -> &'static [Theme] {
        &[Theme::Dark, Theme::Light, Theme::Nord, Theme::Dracula, Theme::Solarized]
    }

    pub fn visuals(&self) -> egui::Visuals {
        match self {
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
            Theme::Dark => egui::Color32::from_rgb(0x63, 0x66, 0xF1),
            Theme::Light => egui::Color32::from_rgb(0x4F, 0x46, 0xE5),
            Theme::Nord => egui::Color32::from_rgb(0x5E, 0x81, 0xAC),
            Theme::Dracula => egui::Color32::from_rgb(0x9C, 0x6B, 0xE8),
            Theme::Solarized => egui::Color32::from_rgb(0x26, 0x8B, 0xD2),
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
// procedurally generated app icon
// -----------------------------------------------------------------------------

pub fn generate_icon() -> Arc<egui::IconData> {
    let size: u32 = 128;
    let s = size as f32;
    let cx = s / 2.0;
    let cy = s / 2.0;
    let r_outer = s / 2.0 - 2.0;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx + 0.5;
            let dy = y as f32 - cy + 0.5;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;

            if dist > r_outer + 1.5 {
                continue;
            }

            let t = (dist / r_outer).clamp(0.0, 1.0);
            let bg_r = (18.0 + 30.0 * t) as u8;
            let bg_g = (30.0 + 80.0 * t) as u8;
            let bg_b = (60.0 + 130.0 * t) as u8;

            let glow = (1.0 - t).clamp(0.0, 1.0);
            let glow_strength = glow * glow * 0.35;

            let ax = dx.abs();
            let ay = dy.abs();
            let star_w = 6.5;
            let star_len = 46.0;
            let in_star =
                (ax < star_w && ay < star_len) || (ay < star_w && ax < star_len);
            let in_center = (ax + ay) < 14.0;

            let mut r =
                (bg_r as f32 * (1.0 - glow_strength) + 120.0 * glow_strength) as u8;
            let mut g =
                (bg_g as f32 * (1.0 - glow_strength) + 200.0 * glow_strength) as u8;
            let mut b =
                (bg_b as f32 * (1.0 - glow_strength) + 255.0 * glow_strength) as u8;

            if in_star || in_center {
                let brightness = 1.0 - (dist / r_outer).powi(2) * 0.55;
                let brightness = brightness.clamp(0.0, 1.0);
                r = (255.0 * brightness + 120.0 * (1.0 - brightness)) as u8;
                g = (255.0 * brightness + 220.0 * (1.0 - brightness)) as u8;
                b = (255.0 * brightness + 255.0 * (1.0 - brightness)) as u8;
            }

            let alpha = if dist > r_outer - 1.5 {
                ((r_outer - dist) / 1.5 * 255.0).clamp(0.0, 255.0) as u8
            } else {
                255
            };

            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = alpha;
        }
    }

    Arc::new(egui::IconData {
        rgba,
        width: size,
        height: size,
    })
}

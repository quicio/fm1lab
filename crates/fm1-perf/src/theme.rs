//! Tema «Ceniza»: oscuro, de escenario, un solo acento.
//!
//! Los tokens son los de `DESIGN.md` y no se inventan acá: si un color no está en esta lista, no existe.
//! Regla que se respeta en todo el dibujo: pelo de 1 px y espacio, nunca cajas ni sombras.

use egui::{Color32, FontFamily, FontId, Stroke, Visuals};

/// Fondo de la ventana.
pub const INK: Color32 = Color32::from_rgb(0x07, 0x08, 0x0A);
/// Superficie de panel (casi igual al fondo: la separación la hace la línea, no el tono).
pub const PANEL: Color32 = Color32::from_rgb(0x0B, 0x0D, 0x10);
/// Pelo de 1 px para separadores.
pub const LINE: Color32 = Color32::from_rgb(0x1B, 0x1F, 0x25);
/// Pelo de 1 px cuando marca estado.
pub const LINE_2: Color32 = Color32::from_rgb(0x2E, 0x34, 0x3C);
pub const TEXT: Color32 = Color32::from_rgb(0xE9, 0xED, 0xF2);
pub const DIM: Color32 = Color32::from_rgb(0xA7, 0xAF, 0xB9);
pub const DIMMER: Color32 = Color32::from_rgb(0x76, 0x7E, 0x88);
/// Lo vivo: paso actual, transporte, conexión, valor recién movido.
pub const SIGNAL: Color32 = Color32::from_rgb(0xC8, 0xF7, 0x51);
/// Error y pánico. Nunca decorativo.
pub const HOT: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x4A);

pub const RADIUS: f32 = 4.0;
pub const HAIRLINE: f32 = 1.0;

/// Ancho de la columna del setlist.
pub const SET_W: f32 = 352.0;
pub const HEADER_H: f32 = 64.0;
pub const FOOTER_H: f32 = 46.0;

fn visuals_dark() -> Visuals {
    let mut v = Visuals::dark();
    v.panel_fill = INK;
    v.window_fill = INK;
    v.extreme_bg_color = PANEL;
    v.faint_bg_color = PANEL;
    v.selection.bg_fill = SIGNAL.linear_multiply(0.22);
    v.selection.stroke = Stroke::new(1.0, SIGNAL);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.inactive.bg_fill = PANEL;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, DIM);
    v.widgets.hovered.bg_fill = PANEL;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.active.bg_fill = PANEL;
    v.widgets.active.fg_stroke = Stroke::new(1.0, SIGNAL);
    v
}

pub fn install(ctx: &egui::Context) {
    ctx.set_visuals(visuals_dark());
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(0.0, 0.0);
    style.spacing.button_padding = egui::vec2(0.0, 0.0);
    style.visuals.window_shadow = egui::epaint::Shadow::NONE;
    style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
    ctx.set_style(style);
}

/// Etiqueta en mayúsculas con tracking. egui no tiene `letter-spacing`, así que se intercala un
/// espacio fino (U+2009): es el mismo truco que usa el mock en CSS.
pub fn tracked(text: &str) -> String {
    let upper = text.to_uppercase();
    let mut out = String::with_capacity(upper.len() * 2);
    for (i, ch) in upper.chars().enumerate() {
        if i > 0 {
            out.push('\u{2009}');
        }
        out.push(ch);
    }
    out
}

pub fn label_font() -> FontId {
    FontId::new(9.5, FontFamily::Proportional)
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Cifra grande de estado: lo que se lee desde el escenario.
pub fn glyph_font() -> FontId {
    FontId::new(104.0, FontFamily::Proportional)
}

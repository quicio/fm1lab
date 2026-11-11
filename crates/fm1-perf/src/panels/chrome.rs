//! Cabecera y pie: el estado del aparato y las ayudas de teclado.

use crate::app::PerfApp;
use crate::link::Link;
use crate::theme;
use crate::widgets::{dot, label, num, ruler, text, text_right};
use eframe::egui::{self, Rect, Ui, pos2, vec2};

pub fn header(app: &PerfApp, ui: &Ui, rect: Rect) {
    let left = rect.left() + 30.0;
    let base = rect.top() + 30.0;

    text(
        ui,
        pos2(left, base),
        &theme::tracked("fm1lab"),
        egui::FontId::new(13.0, egui::FontFamily::Proportional),
        theme::TEXT,
    );
    let mark_w = ui
        .painter()
        .layout_no_wrap(
            theme::tracked("fm1lab"),
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
            theme::TEXT,
        )
        .size()
        .x;
    text(
        ui,
        pos2(left + mark_w + 10.0, base),
        "0.1 · panel",
        theme::mono(10.5),
        theme::DIMMER,
    );

    // estado del enlace
    let stat_x = left + 250.0;
    let alive = matches!(app.link, Link::Ready(_));
    dot(ui, pos2(stat_x, base - 4.0), alive);
    let name = match &app.link {
        Link::Ready(name) => name.clone(),
        Link::Absent => "sin aparato".to_string(),
    };
    text(
        ui,
        pos2(stat_x + 12.0, base),
        &name,
        theme::mono(11.0),
        theme::TEXT,
    );
    text(
        ui,
        pos2(stat_x + 12.0 + 96.0, base),
        "· ch 16 · midi out all",
        theme::mono(11.0),
        theme::DIM,
    );

    // tempo
    num(
        ui,
        pos2(rect.right() - 30.0, base),
        &format!("{:.1}", app.tempo),
        19.0,
        theme::TEXT,
    );
    text_right(
        ui,
        pos2(rect.right() - 30.0 - 52.0, base),
        "bpm",
        theme::mono(9.5),
        theme::DIMMER,
    );
}

/// La regla de marcas, justo debajo de la cabecera.
pub fn ticks(ui: &Ui, rect: Rect) {
    ruler(ui, rect);
}

/// El pelo que cierra la regla de marcas.
pub fn hline_below(ui: &Ui, rect: Rect) {
    crate::widgets::hline(ui, rect.left(), rect.right(), rect.bottom(), theme::LINE);
}

pub fn footer(app: &PerfApp, ui: &Ui, rect: Rect) {
    let base = rect.top() + rect.height() / 2.0 + 4.0;
    let mut x = rect.left() + 30.0;
    for (key, action) in [
        ("1–6", "disparar"),
        ("←→", "control"),
        ("↑↓", "valor"),
        ("±", "tempo"),
        ("esc", "pánico"),
        ("r", "reconectar"),
    ] {
        text(ui, pos2(x, base), key, theme::mono(10.0), theme::DIM);
        let kw = ui
            .painter()
            .layout_no_wrap(key.to_string(), theme::mono(10.0), theme::DIM)
            .size()
            .x;
        text(
            ui,
            pos2(x + kw + 6.0, base),
            action,
            theme::mono(10.0),
            theme::DIMMER,
        );
        let aw = ui
            .painter()
            .layout_no_wrap(action.to_string(), theme::mono(10.0), theme::DIMMER)
            .size()
            .x;
        x += kw + 6.0 + aw + 22.0;
    }

    // a la derecha: qué pasó con el último envío, o el aviso de que no hay ack
    let msg = match &app.status {
        Some(line) => line.clone(),
        None => "sin ack · estado optimista".to_string(),
    };
    let color = if app.error { theme::HOT } else { theme::DIMMER };
    text_right(
        ui,
        pos2(rect.right() - 30.0, base),
        &msg,
        theme::mono(10.0),
        color,
    );
    let _ = label; // el pie usa mono para todo: la etiqueta tracked es para paneles
    let _ = vec2(0.0, 0.0);
}

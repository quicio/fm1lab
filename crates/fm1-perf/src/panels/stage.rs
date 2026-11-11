//! El escenario: qué está sonando, qué viene, el transporte y las cuatro franjas.

use crate::app::PerfApp;
use crate::theme;
use crate::widgets::{StripView, strip, text, text_right};
use eframe::egui::{self, Align2, Rect, Ui, pos2, vec2};

pub fn draw(app: &mut PerfApp, ui: &mut Ui, rect: Rect) {
    let left = rect.left() + 34.0;
    let right = rect.right() - 34.0;

    // ── cifra grande: la sección, que es lo que se lee de lejos ──
    let glyph_base = rect.top() + 122.0;
    let letter = app.current_letter();
    ui.painter().text(
        pos2(left, glyph_base),
        Align2::LEFT_BOTTOM,
        letter.to_string(),
        theme::glyph_font(),
        theme::SIGNAL,
    );
    let glyph_w = ui
        .painter()
        .layout_no_wrap(letter.to_string(), theme::glyph_font(), theme::SIGNAL)
        .size()
        .x;

    // ── al lado: qué proyecto y cuándo se mandó ──
    text(
        ui,
        pos2(left + glyph_w + 20.0, glyph_base - 46.0),
        &theme::tracked(&app.current_project()),
        egui::FontId::new(12.0, egui::FontFamily::Proportional),
        theme::DIM,
    );
    text(
        ui,
        pos2(left + glyph_w + 20.0, glyph_base - 28.0),
        &app.current_subline(),
        theme::mono(11.0),
        theme::DIMMER,
    );

    // ── transporte: el del aparato, no el nuestro ──
    text_right(
        ui,
        pos2(right, glyph_base - 46.0),
        "transporte",
        theme::mono(9.5),
        theme::DIMMER,
    );
    text_right(
        ui,
        pos2(right, glyph_base - 28.0),
        "en el aparato",
        theme::mono(10.0),
        theme::DIM,
    );
    let bar = Rect::from_min_size(pos2(right - 132.0, glyph_base - 14.0), vec2(132.0, 1.0));
    ui.painter().rect_filled(bar, 0.0, theme::LINE_2);

    // ── qué viene, y dónde estamos dentro del set ──
    let cue_y = glyph_base + 36.0;
    text(
        ui,
        pos2(left, cue_y),
        &theme::tracked("siguiente"),
        theme::label_font(),
        theme::DIMMER,
    );
    let next_x = left + 78.0;
    match app.next_label() {
        Some(label) => text(
            ui,
            pos2(next_x, cue_y),
            &label,
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
            theme::DIM,
        ),
        None => text(
            ui,
            pos2(next_x, cue_y),
            "fin del set",
            theme::mono(11.0),
            theme::DIMMER,
        ),
    }

    // dónde estamos dentro del set: un número, no una fila de guiones que hay que interpretar
    let total = app.setlist.len().max(1);
    text_right(
        ui,
        pos2(right, cue_y),
        &format!("{:02} / {:02}", app.current + 1, total),
        theme::mono(10.0),
        theme::DIMMER,
    );

    // ── historial: qué se mandó y hace cuánto. Es la memoria de la sesión ──
    let mut y = cue_y + 44.0;
    crate::widgets::hline(ui, left, right, y - 16.0, theme::LINE);
    text(
        ui,
        pos2(left, y),
        &theme::tracked("últimos envíos"),
        theme::label_font(),
        theme::DIMMER,
    );
    y += 22.0;
    let entries = app.history_view(4);
    if entries.is_empty() {
        text(
            ui,
            pos2(left, y),
            "todavía no se mandó nada en esta sesión",
            theme::mono(10.0),
            theme::DIMMER,
        );
    }
    for slot in 0..4 {
        let Some((when, what)) = entries.get(slot).cloned() else {
            y += 21.0;
            continue;
        };
        text_right(
            ui,
            pos2(left + 64.0, y),
            &when,
            theme::mono(10.0),
            theme::DIMMER,
        );
        text(
            ui,
            pos2(left + 96.0, y),
            &what,
            theme::mono(10.5),
            theme::DIM,
        );
        y += 21.0;
    }

    // ── franjas: cuatro parámetros reales, los que se tocan en vivo ──
    // El alto sale del espacio que queda, no de una constante: así no hay hueco muerto ni recorte.
    let strip_top = y + 10.0;
    let strip_h = ((rect.bottom() - 20.0 - strip_top) / app.strips.len() as f32).clamp(46.0, 88.0);
    // regla de sección: separa «ahora» de «control»
    crate::widgets::hline(ui, left, right, strip_top - 10.0, theme::LINE);
    let mut y = strip_top;
    for index in 0..app.strips.len() {
        let strip_rect = Rect::from_min_size(pos2(left, y), vec2(right - left, strip_h));
        let (name, value, frac) = app.strip_view(index);
        let focused = index == app.focus;
        let touched = app.is_touched(index);
        let view = StripView {
            name: &name,
            value: &value,
            frac,
            bipolar: app.is_bipolar(index),
            focused,
            touched,
        };
        let response = strip(ui, strip_rect, &view);
        if response.clicked() {
            app.focus = index;
        }
        y += strip_h;
    }
}

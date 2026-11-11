//! Piezas de dibujo. Todo es pelo de 1 px, texto y espacio: no hay cajas.

use crate::theme;
use eframe::egui::{self, Align2, Color32, FontId, Rect, Response, Sense, Stroke, Ui, pos2, vec2};

pub fn hline(ui: &Ui, x0: f32, x1: f32, y: f32, color: Color32) {
    ui.painter()
        .hline(x0..=x1, y, Stroke::new(theme::HAIRLINE, color));
}

pub fn vline(ui: &Ui, x: f32, y0: f32, y1: f32, color: Color32) {
    ui.painter()
        .vline(x, y0..=y1, Stroke::new(theme::HAIRLINE, color));
}

/// Etiqueta en mayúsculas con tracking, alineada a la izquierda de `pos`.
pub fn label(ui: &Ui, pos: egui::Pos2, text: &str, color: Color32) {
    ui.painter().text(
        pos,
        Align2::LEFT_BOTTOM,
        theme::tracked(text),
        theme::label_font(),
        color,
    );
}

pub fn num(ui: &Ui, pos: egui::Pos2, text: &str, size: f32, color: Color32) {
    ui.painter()
        .text(pos, Align2::RIGHT_BOTTOM, text, theme::mono(size), color);
}

/// Regla de marcas: 1 px cada 12, más alto cada 60. La única decoración permitida, y es escala.
pub fn ruler(ui: &Ui, rect: Rect) {
    let painter = ui.painter();
    let mut x = rect.left();
    let mut n = 0;
    while x < rect.right() {
        let tall = n % 5 == 0;
        let h = if tall {
            rect.height()
        } else {
            rect.height() * 0.55
        };
        painter.vline(
            x,
            (rect.bottom() - h)..=rect.bottom(),
            Stroke::new(1.0, if tall { theme::LINE_2 } else { theme::LINE }),
        );
        x += 12.0;
        n += 1;
    }
}

/// Una franja de parámetro: etiqueta, línea de 2 px con relleno, y valor en mono tabular.
///
/// `frac` es 0..1 sobre el rango. Si el rango cruza el cero (filtros, pitch), el relleno sale del centro
/// hacia el valor y se marca el cero: así el dibujo dice lo mismo que el número.
/// Todo lo que una franja necesita para dibujarse, en un solo objeto: ocho argumentos sueltos se leen mal.
pub struct StripView<'a> {
    pub name: &'a str,
    pub value: &'a str,
    /// 0..1 sobre el rango del parámetro, ya normalizado.
    pub frac: f32,
    pub bipolar: bool,
    pub focused: bool,
    pub touched: bool,
}

pub fn strip(ui: &mut Ui, rect: Rect, view: &StripView) -> Response {
    let StripView {
        name,
        value,
        frac,
        bipolar,
        focused,
        touched,
    } = *view;
    let response = ui.interact(rect, ui.id().with(name), Sense::click());
    hline(ui, rect.left(), rect.right(), rect.top(), theme::LINE);

    let base = rect.bottom() - 16.0;
    let label_color = if focused { theme::TEXT } else { theme::DIM };
    label(ui, pos2(rect.left(), base), name, label_color);
    if focused {
        ui.painter().rect_filled(
            Rect::from_min_size(pos2(rect.left() - 12.0, base - 9.0), vec2(2.0, 11.0)),
            0.0,
            theme::DIM,
        );
    }

    let track = Rect::from_min_size(
        pos2(rect.left() + 118.0, base - 1.0),
        vec2(rect.width() - 118.0 - 64.0, 2.0),
    );
    let painter = ui.painter();
    painter.rect_filled(track, 0.0, theme::LINE_2);

    let color = if touched { theme::SIGNAL } else { theme::DIM };
    let x_of = |f: f32| track.left() + track.width() * f.clamp(0.0, 1.0);
    if bipolar {
        let center = x_of(0.5);
        painter.vline(
            center,
            (track.top() - 3.0)..=(track.bottom() + 3.0),
            Stroke::new(1.0, theme::LINE_2.linear_multiply(1.6)),
        );
        let edge = x_of(frac);
        let segment = Rect::from_x_y_ranges(
            center.min(edge)..=center.max(edge),
            track.top()..=track.bottom(),
        );
        painter.rect_filled(segment, 0.0, color);
    } else {
        painter.rect_filled(
            Rect::from_min_size(
                track.min,
                vec2((track.width() * frac.clamp(0.0, 1.0)).max(1.0), 2.0),
            ),
            0.0,
            color,
        );
    }

    num(
        ui,
        pos2(rect.right(), base),
        value,
        12.0,
        if touched { theme::SIGNAL } else { theme::TEXT },
    );
    response
}

/// Punto de estado: la única forma de decir "hay aparato" sin escribir una frase.
pub fn dot(ui: &Ui, center: egui::Pos2, alive: bool) {
    let painter = ui.painter();
    let color = if alive { theme::SIGNAL } else { theme::HOT };
    painter.circle_filled(center, 2.5, color);
    painter.circle_stroke(center, 5.5, Stroke::new(1.0, color.linear_multiply(0.25)));
}

/// Texto suelto con fuente y color explícitos, alineado a la izquierda de `pos`.
pub fn text(ui: &Ui, pos: egui::Pos2, text: &str, font: FontId, color: Color32) {
    ui.painter()
        .text(pos, Align2::LEFT_BOTTOM, text, font, color);
}

/// Igual que [`text`], pero alineado a la derecha.
pub fn text_right(ui: &Ui, pos: egui::Pos2, text: &str, font: FontId, color: Color32) {
    ui.painter()
        .text(pos, Align2::RIGHT_BOTTOM, text, font, color);
}

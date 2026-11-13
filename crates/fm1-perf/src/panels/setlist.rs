//! La lista del show. Cada fila es un disparo.

use crate::app::PerfApp;
use crate::theme;
use crate::widgets::{hline, label, text, text_right};
use eframe::egui::{self, Rect, Sense, Ui, pos2, vec2};

const ROW_H: f32 = 44.0;

pub fn draw(app: &mut PerfApp, ui: &mut Ui, rect: Rect) {
    let head_y = rect.top() + 24.0;
    label(
        ui,
        pos2(rect.left() + 30.0, head_y),
        &format!("set · {}", app.setlist.name),
        theme::DIMMER,
    );
    text_right(
        ui,
        pos2(rect.right() - 18.0, head_y),
        &format!("{} pasos", app.setlist.len()),
        theme::mono(9.5),
        theme::DIMMER,
    );

    let mut y = rect.top() + 46.0;
    let items = app.setlist.items.clone();
    for (index, item) in items.iter().enumerate() {
        let row = Rect::from_min_size(
            pos2(rect.left() + 18.0, y),
            vec2(rect.width() - 36.0, ROW_H),
        );
        if row.bottom() > rect.bottom() - 6.0 {
            break;
        }
        let response = ui.interact(row, ui.id().with(("step", index)), Sense::click());
        let active = index == app.current;
        let hovered = response.hovered();

        if active {
            hue(ui, row);
        } else if hovered {
            ui.painter()
                .rect_filled(row, theme::RADIUS, theme::PANEL.linear_multiply(0.9));
        }

        text(
            ui,
            pos2(row.left() + 12.0, row.center().y + 4.0),
            &format!("{:02}", index + 1),
            theme::mono(10.0),
            if active { theme::TEXT } else { theme::DIMMER },
        );
        text(
            ui,
            pos2(row.left() + 46.0, row.center().y + 5.0),
            &item.label(),
            egui::FontId::new(13.5, egui::FontFamily::Proportional),
            if active || hovered {
                theme::TEXT
            } else {
                theme::DIM
            },
        );

        if hovered && !active {
            text_right(
                ui,
                pos2(row.right() - 12.0, row.center().y + 4.0),
                &format!("{}", index + 1),
                theme::mono(9.5),
                theme::DIMMER,
            );
        }

        hline(
            ui,
            row.left(),
            row.right(),
            row.bottom(),
            theme::LINE.linear_multiply(0.7),
        );

        if response.clicked() {
            app.fire(index);
        }
        y += ROW_H;
    }

    if items.is_empty() {
        let message = if app.has_scenes() {
            "sin pasos · el aparato no tiene secciones guardadas · r relee"
        } else {
            "sin pasos · este perfil no declara escenas"
        };
        text(
            ui,
            pos2(rect.left() + 30.0, rect.top() + 80.0),
            message,
            theme::mono(10.0),
            theme::DIMMER,
        );
    }
}

/// La marca de "esto está sonando": barra de 2 px y un velo que se apaga hacia la derecha.
fn hue(ui: &Ui, row: Rect) {
    let painter = ui.painter();
    painter.rect_filled(
        Rect::from_min_size(row.min, vec2(2.0, row.height())),
        0.0,
        theme::SIGNAL,
    );
    // sin gradientes en egui: se simula con cuatro velos decrecientes, imperceptibles por separado
    for step in 0..4 {
        let t = step as f32;
        let width = row.width() * (1.0 - t * 0.22);
        let alpha = 18 - step * 4;
        if alpha <= 0 {
            break;
        }
        let color = egui::Color32::from_rgba_unmultiplied(
            theme::SIGNAL.r(),
            theme::SIGNAL.g(),
            theme::SIGNAL.b(),
            alpha as u8,
        );
        painter.rect_filled(
            Rect::from_min_size(row.min + vec2(2.0, 0.0), vec2(width, row.height())),
            0.0,
            color,
        );
    }
}

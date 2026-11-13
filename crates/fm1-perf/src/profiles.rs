//! Selector de perfil y armado de franjas: lo que la app necesita para cambiar de firmware sin
//! reiniciar. No habla MIDI; sólo prepara datos y dibuja una ventana modal.

use crate::app::{Origin, Strip, demo_setlist};
use crate::link::Link;
use crate::theme;
use eframe::egui;
use eframe::egui::vec2;
use fm1_core::Scope;
use fm1_core::profile::{Param, Profile, SceneSpec, TransportSpec};
use fm1_core::setlist::Setlist;
use fm1_store::Panel;
use std::path::Path;

pub enum Action {
    None,
    Close,
    Choose(Profile),
}

/// Las cuatro franjas del perfil: primero lo guardado en `panel.json`, después los defaults y el resto
/// del catálogo. Devuelve los ids guardados que ya no existen, para avisarlos.
pub fn strips_for(profile: &Profile, panel: &Panel) -> (Vec<Strip>, Vec<String>) {
    let mut params: Vec<Param> = Vec::new();
    let mut problems = Vec::new();
    let saved = panel.get(&profile.id).cloned().unwrap_or_default();
    for id in &saved {
        if id.is_empty() {
            continue;
        }
        match profile.param(id) {
            Some(param) if params.len() < 4 => params.push(param.clone()),
            Some(_) => {}
            None => problems.push(format!("franja {id} no existe en {}", profile.id)),
        }
    }
    for id in &profile.default_strips {
        if params.len() >= 4 {
            break;
        }
        if !params.iter().any(|param| &param.id == id)
            && let Some(param) = profile.param(id)
        {
            params.push(param.clone());
        }
    }
    for param in &profile.params {
        if params.len() >= 4 {
            break;
        }
        if !params.iter().any(|existing| existing.id == param.id) {
            params.push(param.clone());
        }
    }
    let jiant = profile.id == "jiant-par12";
    let strips = params
        .into_iter()
        .take(4)
        .map(|param| Strip {
            value: param.default,
            label: strip_label(&param, 0, jiant),
            param,
        })
        .collect();
    (strips, problems)
}

/// Dibuja la lista de perfiles built-in + usuario. La app decide qué hacer con la elección.
pub fn draw(ctx: &egui::Context, profiles: &[Profile], current: &str, link: &Link) -> Action {
    let mut close = false;
    let mut chosen: Option<Profile> = None;
    let screen = ctx.screen_rect();
    let width = (screen.width() - 60.0).clamp(480.0, 640.0);
    let height = (screen.height() - 80.0).clamp(320.0, 540.0);
    let frame = egui::Frame::window(&ctx.style())
        .fill(theme::PANEL)
        .stroke(egui::Stroke::new(theme::HAIRLINE, theme::LINE_2))
        .inner_margin(egui::Margin::same(14));
    let device = match link {
        Link::Ready(name) => format!("aparato: {name}"),
        Link::Absent => "aparato: sin respuesta".to_string(),
    };

    egui::Window::new("perfil del aparato")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([width, height])
        .collapsible(false)
        .resizable(false)
        .frame(frame)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            ui.spacing_mut().button_padding = vec2(8.0, 4.0);
            ui.set_width(width - 30.0);
            ui.add(egui::Label::new(egui::RichText::new(device).color(theme::DIM)).truncate());
            ui.label(
                egui::RichText::new("elegí un perfil · se recuerda para la próxima apertura")
                    .color(theme::DIMMER)
                    .size(10.5),
            );
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("profile-list")
                .max_height(height - 150.0)
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        for profile in profiles {
                            let active = profile.id == current;
                            let marker = if active { "• " } else { "  " };
                            let response = ui.selectable_label(
                                active,
                                egui::RichText::new(format!("{marker}{}", profile.name))
                                    .size(14.0)
                                    .color(if active { theme::SIGNAL } else { theme::TEXT }),
                            );
                            if response.clicked() {
                                chosen = Some(profile.clone());
                            }
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!(
                                        "{} · {} params · {}",
                                        profile.id,
                                        profile.params.len(),
                                        transport_name(&profile.transport)
                                    ))
                                    .size(10.5)
                                    .color(theme::DIMMER),
                                )
                                .truncate(),
                            );
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(scene_name(&profile.scenes))
                                        .size(10.5)
                                        .color(theme::DIMMER),
                                )
                                .truncate(),
                            );
                            ui.add_space(4.0);
                        }
                    });
                });
            if ui.button("cerrar").clicked() {
                close = true;
            }
        });

    if let Some(profile) = chosen {
        Action::Choose(profile)
    } else if close {
        Action::Close
    } else {
        Action::None
    }
}

fn transport_name(spec: &TransportSpec) -> &'static str {
    match spec {
        TransportSpec::ClockMaster { .. } => "clock master",
        TransportSpec::Midi => "clock MIDI",
        TransportSpec::None => "sin transporte",
    }
}

fn scene_name(spec: &SceneSpec) -> &'static str {
    match spec {
        SceneSpec::ProgramChange { .. } => "escenas: secciones/proyectos",
        SceneSpec::Patterns { .. } => "escenas: patterns",
        SceneSpec::None => "sin escenas",
    }
}

/// Etiqueta de la franja: en JIANT, los parámetros de pista llevan el número de pista del aparato.
pub fn strip_label(param: &Param, selected: u8, jiant: bool) -> String {
    if jiant && param.scope == Scope::Track {
        format!("t{} {}", selected + 1, param.label.to_lowercase())
    } else {
        param.label.to_lowercase()
    }
}

pub fn load_setlist_for(
    profile: &Profile,
    path: &Path,
    demo: bool,
) -> (Setlist, Origin, Option<String>, bool) {
    if demo {
        return (demo_setlist(), Origin::File, None, false);
    }
    if profile.id != "jiant-par12" {
        return (Setlist::new(&profile.name), Origin::File, None, false);
    }
    match fm1_store::load(path) {
        Ok(set) => {
            let steps = set.len();
            (
                set,
                Origin::File,
                Some(format!("cargado · {steps} pasos")),
                false,
            )
        }
        Err(fm1_store::StoreError::Missing(_)) => {
            (Setlist::new("aparato"), Origin::Device, None, false)
        }
        Err(error) => (
            Setlist::new("aparato"),
            Origin::Device,
            Some(format!("no se pudo cargar: {error}")),
            true,
        ),
    }
}

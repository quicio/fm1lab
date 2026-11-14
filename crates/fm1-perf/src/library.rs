//! La biblioteca de firmwares: catálogo buscable, estado del paquete y backup/instalación.
//!
//! La ventana no habla MIDI: pide el flash a un hilo y muestra su progreso. La app para el transporte y
//! el monitor antes de llamar a [`Library::start_install`]; acá sólo se verifica el paquete, se respalda
//! y se corre el instalador.

use crate::theme;
use eframe::egui;
use eframe::egui::vec2;
use fm1_core::route::{self, Family, PlanStep, Refusal, StepKind, Target};
use fm1_store::{Catalog, FileState, FirmwareEntry, state_of};
use std::collections::{BTreeMap, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

enum Msg {
    Line(String),
    Progress { end: usize, total: usize },
    Installed(std::result::Result<String, String>),
    BackedUp(std::result::Result<String, String>),
    Flashed(std::result::Result<String, String>),
}

pub struct Library {
    open: bool,
    demo: bool,
    catalog: Catalog,
    states: BTreeMap<String, FileState>,
    query: String,
    selected: Option<String>,
    confirm: bool,
    installed: Option<String>,
    installed_error: Option<String>,
    probing: bool,
    busy: Option<String>,
    flashed: bool,
    progress: Option<(usize, usize)>,
    log: VecDeque<String>,
    notice: Option<(String, bool)>,
    result: Option<(String, bool)>,
    focus_search: bool,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl Library {
    /// En `--demo` no se sondea el aparato: la captura queda determinista.
    pub fn new(demo: bool) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut library = Self {
            open: false,
            demo,
            catalog: Catalog::load(),
            states: BTreeMap::new(),
            query: String::new(),
            selected: None,
            confirm: false,
            installed: None,
            installed_error: None,
            probing: false,
            busy: None,
            flashed: false,
            progress: None,
            log: VecDeque::new(),
            notice: None,
            result: None,
            focus_search: false,
            tx,
            rx,
        };
        library.rehash();
        if demo {
            // captura determinista: sin sondeo, pero con un aparato plausible y una entrada de otra
            // familia seleccionada, para que la ruta de dos pasos quede dibujada
            library.installed = Some("FM-1_9010202".to_string());
            library.selected = library
                .catalog
                .entries
                .iter()
                .find(|entry| entry.id == "jiant-v10.7.23")
                .or_else(|| {
                    library
                        .catalog
                        .entries
                        .iter()
                        .find(|entry| Family::parse(&entry.family) != Family::X0x)
                })
                .map(|entry| entry.id.clone());
        } else {
            library.probe();
        }
        library
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Si hay un flash en curso: la app mantiene el monitor en pausa hasta que termine.
    pub fn is_busy(&self) -> bool {
        self.busy.is_some()
    }

    /// `true` una sola vez, cuando un flash terminó bien: la app re-detecta el perfil.
    pub fn take_flash_completion(&mut self) -> bool {
        std::mem::take(&mut self.flashed)
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.focus_search = true;
            self.refresh();
            if self.selected.is_none() {
                self.selected = self.catalog.entries.first().map(|entry| entry.id.clone());
            }
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Selecciona una entrada por id (para `--firmware` y pruebas visuales).
    pub fn select(&mut self, id: &str) {
        if self.catalog.get(id).is_some() {
            self.selected = Some(id.to_string());
        }
    }

    /// Relee el catálogo de usuario y vuelve a hashear los paquetes.
    pub fn refresh(&mut self) {
        self.catalog = Catalog::load();
        self.rehash();
        self.probe();
    }

    fn rehash(&mut self) {
        self.states = self
            .catalog
            .entries
            .iter()
            .map(|entry| (entry.id.clone(), state_of(entry)))
            .collect();
        if self
            .selected
            .as_ref()
            .is_some_and(|id| self.catalog.get(id).is_none())
        {
            self.selected = None;
        }
    }

    fn probe(&mut self) {
        if self.demo || self.probing {
            return;
        }
        self.probing = true;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = fm1_flash::Installer::new(None)
                .probe()
                .map(|(identity, _port)| identity.text)
                .map_err(|error| error.to_string());
            let _ = tx.send(Msg::Installed(result));
        });
    }

    pub fn backup(&mut self) {
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = fm1_store::backup_lab()
                .map(|report| {
                    format!(
                        "backup en {} · {} archivos",
                        report.dir.display(),
                        report.files.len()
                    )
                })
                .map_err(|error| error.to_string());
            let _ = tx.send(Msg::BackedUp(result));
        });
    }

    /// Respalda y flashea en un hilo. La app ya paró el transporte y cerró el monitor.
    pub fn start_install(&mut self, entry: FirmwareEntry) {
        if self.busy.is_some() {
            return;
        }
        self.busy = Some(entry.name.clone());
        self.progress = None;
        self.result = None;
        self.log.clear();
        let tx = self.tx.clone();
        thread::spawn(move || {
            // El sondeo/monitor pueden estar a mitad de una lectura: se les da tiempo a soltar el
            // puerto antes de que el instalador empiece el handshake.
            let _ = tx.send(Msg::Line("preparando el puerto…".into()));
            thread::sleep(Duration::from_millis(900));
            let _ = tx.send(Msg::Line(
                "backup de configuración y paquetes antes de tocar la flash…".into(),
            ));
            match fm1_store::backup_lab() {
                Ok(report) => {
                    let _ = tx.send(Msg::Line(format!(
                        "backup: {} · {} archivos",
                        report.dir.display(),
                        report.files.len()
                    )));
                    for error in &report.errors {
                        let _ = tx.send(Msg::Line(format!("backup parcial: {error}")));
                    }
                }
                Err(error) => {
                    let _ = tx.send(Msg::Flashed(Err(format!(
                        "no se pudo respaldar; no se escribió nada: {error}"
                    ))));
                    return;
                }
            }

            let installer = fm1_flash::Installer::new(None);
            // La ruta se decide con lo que el aparato reporta ahora, no con el sondeo de hace un rato.
            let current = match installer.probe() {
                Ok((identity, port)) => {
                    let _ = tx.send(Msg::Line(format!("aparato: {} ({port})", identity.text)));
                    identity.text
                }
                Err(error) => {
                    let _ = tx.send(Msg::Flashed(Err(error.to_string())));
                    return;
                }
            };
            let catalog = Catalog::load();
            let stock = catalog
                .entries
                .iter()
                .find(|other| Family::parse(&other.family) == Family::Stock)
                .cloned();
            let target = Target::new(
                entry.id.clone(),
                entry.identity.clone(),
                Family::parse(&entry.family),
            );
            let steps = match route::plan(&current, &target) {
                Ok(steps) => steps,
                Err(refusal) => {
                    let _ = tx.send(Msg::Flashed(Err(refusal.to_string())));
                    return;
                }
            };
            let _ = tx.send(Msg::Line(format!(
                "ruta: {} paso(s) desde {current}",
                steps.len()
            )));

            for (index, step) in steps.iter().enumerate() {
                let (path, step_force, what) = match step.kind {
                    StepKind::Stock => match &stock {
                        Some(other) => (other.path(), true, "oficial"),
                        None => {
                            let _ = tx.send(Msg::Flashed(Err(Refusal::NoStock.to_string())));
                            return;
                        }
                    },
                    StepKind::Target => (entry.path(), false, "destino"),
                };
                let _ = tx.send(Msg::Line(format!(
                    "[{}/{}] {what}: {}",
                    index + 1,
                    steps.len(),
                    path.display()
                )));
                let package = match fm1_flash::Package::load(&path, step_force) {
                    Ok(package) => package,
                    Err(error) => {
                        let _ = tx.send(Msg::Flashed(Err(error.to_string())));
                        return;
                    }
                };
                let _ = tx.send(Msg::Line(format!(
                    "identidad del paquete: {} · {} bytes",
                    package.product,
                    package.image.len()
                )));
                let sender = tx.clone();
                let written = installer.install(&package, &mut |event| match event {
                    fm1_flash::Event::Start(text) => {
                        let _ = sender.send(Msg::Line(format!("handshake con {text}")));
                    }
                    fm1_flash::Event::Loader => {
                        let _ = sender.send(Msg::Line("loader: modo update".into()));
                    }
                    fm1_flash::Event::Progress { end, total, .. } => {
                        let _ = sender.send(Msg::Progress { end, total });
                    }
                    fm1_flash::Event::WriteDone => {
                        let _ = sender.send(Msg::Line("escritura completa".into()));
                    }
                    fm1_flash::Event::Reboot => {
                        let _ = sender.send(Msg::Line("reiniciando…".into()));
                    }
                    fm1_flash::Event::Done(text) => {
                        let _ = sender.send(Msg::Line(format!("verificado: {text}")));
                    }
                });
                match written {
                    Ok(identity) if identity.text == step.expect => {
                        let _ = tx.send(Msg::Line(format!(
                            "paso {} ok: {}",
                            index + 1,
                            identity.text
                        )));
                    }
                    Ok(identity) => {
                        let _ = tx.send(Msg::Flashed(Err(format!(
                            "después del paso {} el aparato dice {}, se esperaba {}",
                            index + 1,
                            identity.text,
                            step.expect
                        ))));
                        return;
                    }
                    Err(error) => {
                        let _ = tx.send(Msg::Flashed(Err(error.to_string())));
                        return;
                    }
                }
            }
            if steps.len() > 1 {
                let _ = tx.send(Msg::Line(
                    "nota: los proyectos del firmware anterior se perdieron al pasar por el oficial"
                        .into(),
                ));
            }
            let _ = tx.send(Msg::Flashed(Ok(entry.identity.clone())));
        });
    }

    /// Procesa lo que mandaron los hilos. Devuelve algo para el pie de la app.
    pub fn poll(&mut self) -> Option<(String, bool)> {
        let mut status = None;
        while let Ok(message) = self.rx.try_recv() {
            match message {
                Msg::Line(line) => {
                    self.log.push_back(line);
                    if self.log.len() > 40 {
                        self.log.pop_front();
                    }
                }
                Msg::Progress { end, total } => self.progress = Some((end, total)),
                Msg::Installed(Ok(identity)) => {
                    self.probing = false;
                    if self.selected.is_none() {
                        self.selected = self
                            .catalog
                            .entries
                            .iter()
                            .find(|entry| entry.identity == identity)
                            .map(|entry| entry.id.clone())
                            .or_else(|| self.catalog.entries.first().map(|entry| entry.id.clone()));
                    }
                    self.installed = Some(identity);
                    self.installed_error = None;
                }
                Msg::Installed(Err(error)) => {
                    self.probing = false;
                    self.installed_error = Some(error);
                }
                Msg::BackedUp(result) => {
                    let (line, error) = match result {
                        Ok(line) => (line, false),
                        Err(error) => (format!("backup: {error}"), true),
                    };
                    self.notice = Some((line.clone(), error));
                    status = Some((line, error));
                }
                Msg::Flashed(Ok(identity)) => {
                    self.busy = None;
                    self.progress = None;
                    self.flashed = true;
                    self.installed = Some(identity.clone());
                    self.installed_error = None;
                    let line = format!("instalado · {identity}");
                    self.result = Some((line.clone(), false));
                    status = Some((line, false));
                }
                Msg::Flashed(Err(error)) => {
                    self.busy = None;
                    self.progress = None;
                    self.result = Some((format!("flash: {error}"), true));
                    status = Some((format!("flash: {error}"), true));
                }
            }
        }
        status
    }

    /// La ruta segura para esta entrada, o el motivo por el que no se escribe. `None` sin sondeo.
    fn planned(
        &self,
        entry: &FirmwareEntry,
    ) -> Option<std::result::Result<Vec<PlanStep>, Refusal>> {
        let current = self.installed.as_deref()?;
        let target = Target::new(
            entry.id.clone(),
            entry.identity.clone(),
            Family::parse(&entry.family),
        );
        Some(route::plan(current, &target))
    }

    /// El nombre del firmware instalado, si el catálogo lo reconoce por identidad.
    fn installed_entry(&self) -> Option<&FirmwareEntry> {
        let identity = self.installed.as_deref()?;
        self.catalog
            .entries
            .iter()
            .find(|entry| entry.identity == identity)
    }

    /// Dibuja la ventana. Devuelve la entrada a instalar cuando el usuario confirma.
    pub fn draw(&mut self, ctx: &egui::Context) -> Option<FirmwareEntry> {
        if !self.open {
            return None;
        }
        let mut action: Option<FirmwareEntry> = None;
        let mut close = false;
        let mut refresh = false;
        let mut backup = false;

        let frame = egui::Frame::window(&ctx.style())
            .fill(theme::PANEL)
            .stroke(egui::Stroke::new(theme::HAIRLINE, theme::LINE_2))
            .inner_margin(egui::Margin::same(14));

        // Se adapta a la ventana: en 900×560 la biblioteca sigue entrando.
        let screen = ctx.screen_rect();
        let width = (screen.width() - 40.0).clamp(760.0, 1000.0);
        let height = (screen.height() - 40.0).clamp(440.0, 620.0);
        let list_w = (width * 0.44).max(300.0);
        let detail_w = width - list_w - 40.0;
        let pane_h = height - 160.0;

        egui::Window::new("biblioteca de firmwares")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([width, height])
            .collapsible(false)
            .resizable(false)
            .frame(frame)
            .show(ctx, |ui| {
                // El panel global usa espaciado cero para dibujar a mano; la ventana no.
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                ui.spacing_mut().button_padding = vec2(8.0, 4.0);
                ui.set_width(width - 30.0);
                ui.set_max_width(width - 30.0);

                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .hint_text("buscar por nombre, familia, versión o etiqueta…")
                            .desired_width(520.0),
                    );
                    if self.focus_search {
                        response.request_focus();
                        self.focus_search = false;
                    }
                    if ui.button("actualizar").clicked() {
                        refresh = true;
                    }
                    if ui.button("backup").clicked() {
                        backup = true;
                    }
                    if ui.button("cerrar").clicked() {
                        close = true;
                    }
                });

                let device = if let Some(identity) = &self.installed {
                    let name = self
                        .installed_entry()
                        .map(|entry| entry.name.clone())
                        .unwrap_or_else(|| "firmware no catalogado".into());
                    format!("aparato: {identity} · {name}")
                } else if let Some(error) = &self.installed_error {
                    format!("aparato: sin respuesta · {error}")
                } else if self.demo {
                    "aparato: demo (sin sondeo)".into()
                } else if self.probing {
                    "aparato: preguntando identidad…".into()
                } else {
                    "aparato: desconocido".into()
                };
                ui.add(
                    egui::Label::new(egui::RichText::new(device).color(theme::DIM))
                        .wrap_mode(egui::TextWrapMode::Truncate),
                );
                for error in &self.catalog.errors {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("catálogo: {error}")).color(theme::HOT),
                        )
                        .wrap(),
                    );
                }
                ui.separator();

                let matches: Vec<FirmwareEntry> = self
                    .catalog
                    .search(&self.query)
                    .into_iter()
                    .cloned()
                    .collect();
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        vec2(list_w, pane_h),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.set_width(list_w - 10.0);
                            egui::ScrollArea::vertical()
                                .id_salt("firmware-list")
                                .max_height(pane_h - 10.0)
                                .show(ui, |ui| {
                                    for entry in &matches {
                                        let state = self.states.get(&entry.id);
                                        let installed =
                                            self.installed.as_deref() == Some(&entry.identity);
                                        let marker = if installed { "• " } else { "  " };
                                        let response = ui.selectable_label(
                                            self.selected.as_deref() == Some(entry.id.as_str()),
                                            egui::RichText::new(format!("{marker}{}", entry.name))
                                                .size(14.0)
                                                .color(if installed {
                                                    theme::SIGNAL
                                                } else {
                                                    theme::TEXT
                                                }),
                                        );
                                        if response.clicked() {
                                            self.selected = Some(entry.id.clone());
                                            self.confirm = false;
                                        }
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(format!(
                                                    "v{} · {}",
                                                    entry.version, entry.family
                                                ))
                                                .size(11.0)
                                                .color(theme::DIM),
                                            )
                                            .truncate(),
                                        );
                                        if let Some(state) = state {
                                            let color = if state.is_ready() {
                                                theme::DIMMER
                                            } else {
                                                theme::HOT
                                            };
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(state.label())
                                                        .size(10.5)
                                                        .color(color),
                                                )
                                                .truncate(),
                                            );
                                        }
                                        ui.add_space(4.0);
                                    }
                                });
                        },
                    );
                    ui.separator();
                    ui.allocate_ui_with_layout(
                        vec2(detail_w, pane_h),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("firmware-detail")
                                .max_height(pane_h)
                                .show(ui, |ui| {
                                    ui.set_width(detail_w - 30.0);
                                    let Some(entry) = self
                                        .selected
                                        .as_ref()
                                        .and_then(|id| self.catalog.get(id))
                                        .cloned()
                                    else {
                                        ui.label("elegí un firmware de la lista");
                                        return;
                                    };
                                    ui.heading(&entry.name);
                                    ui.label(
                                        egui::RichText::new(entry.subtitle()).color(theme::DIM),
                                    );
                                    ui.add(
                                        egui::Label::new(format!(
                                            "identidad OTA: {}",
                                            entry.identity
                                        ))
                                        .truncate(),
                                    );
                                    ui.add(
                                        egui::Label::new(format!(
                                            "archivo: {}",
                                            entry.path().display()
                                        ))
                                        .truncate(),
                                    );
                                    if let Some(profile) = &entry.profile {
                                        ui.label(format!("perfil sugerido: {profile}"));
                                    }
                                    if let Some(url) = &entry.url {
                                        ui.hyperlink_to("origen", url);
                                    }
                                    ui.separator();
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&entry.notes).color(theme::DIM),
                                        )
                                        .wrap(),
                                    );
                                    ui.separator();

                                    let state = self.states.get(&entry.id);
                                    match state {
                                        Some(state) => {
                                            let color = if state.is_ready() {
                                                theme::SIGNAL
                                            } else {
                                                theme::HOT
                                            };
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(state.label()).color(color),
                                                )
                                                .wrap(),
                                            );
                                        }
                                        None => {
                                            ui.label(
                                                egui::RichText::new("sin verificar")
                                                    .color(theme::HOT),
                                            );
                                        }
                                    }
                                    let ready = state.is_some_and(FileState::is_ready);
                                    let installed =
                                        self.installed.as_deref() == Some(&entry.identity);
                                    if installed {
                                        ui.label(
                                            egui::RichText::new("esta es la versión instalada")
                                                .color(theme::SIGNAL),
                                        );
                                    } else if self.installed.as_deref().is_some_and(|identity| {
                                        self.installed_entry()
                                            .is_some_and(|other| other.family == entry.family)
                                            && identity != entry.identity
                                    }) {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "otra versión de {} está instalada",
                                                entry.family
                                            ))
                                            .color(theme::DIMMER),
                                        );
                                    }

                                    // La ruta va antes que el botón: si no se puede, no hay botón.
                                    let planned = self.planned(&entry);
                                    let step_count = planned
                                        .as_ref()
                                        .and_then(|plan| plan.as_ref().ok())
                                        .map(Vec::len)
                                        .unwrap_or(1);
                                    if let Some(plan) = &planned {
                                        ui.add_space(6.0);
                                        match plan {
                                            Ok(steps) => {
                                                let title = if steps.len() == 1 {
                                                    "ruta directa · 1 paso".to_string()
                                                } else {
                                                    format!(
                                                        "ruta segura · {} pasos automáticos",
                                                        steps.len()
                                                    )
                                                };
                                                ui.label(
                                                    egui::RichText::new(theme::tracked(&title))
                                                        .size(9.5)
                                                        .color(theme::DIMMER),
                                                );
                                                if steps.len() > 1 {
                                                    ui.label(
                                                        egui::RichText::new(
                                                            "un clic ejecuta la ruta completa; no toques el aparato entre pasos",
                                                        )
                                                        .size(10.0)
                                                        .color(theme::SIGNAL),
                                                    );
                                                }
                                                for (index, step) in steps.iter().enumerate() {
                                                    let what = match step.kind {
                                                        StepKind::Stock => "oficial",
                                                        StepKind::Target => "destino",
                                                    };
                                                    ui.label(
                                                        egui::RichText::new(format!(
                                                            "{}. {what}: espera {}\n     {}",
                                                            index + 1,
                                                            step.expect,
                                                            step.reason
                                                        ))
                                                        .size(10.5)
                                                        .color(theme::DIM),
                                                    );
                                                }
                                                let from = Family::from_identity(
                                                    self.installed.as_deref().unwrap_or(""),
                                                );
                                                let target_family = Family::parse(&entry.family);
                                                let cross = from != target_family
                                                    && from != Family::Stock
                                                    && target_family != Family::Stock;
                                                if cross {
                                                    ui.label(
                                                        egui::RichText::new(
                                                            "se pierde: los proyectos y presets del firmware actual",
                                                        )
                                                        .size(10.5)
                                                        .color(theme::HOT),
                                                    );
                                                }
                                            }
                                            Err(refusal) => {
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "no se puede: {refusal}"
                                                    ))
                                                    .size(10.5)
                                                    .color(theme::HOT),
                                                );
                                            }
                                        }
                                    }
                                    let blocked = matches!(planned, Some(Err(_)));

                                    ui.add_space(8.0);
                                    ui.add_enabled_ui(ready && self.busy.is_none(), |ui| {
                                        ui.checkbox(
                                            &mut self.confirm,
                                            "confirmo: se reescribe la flash; no desconectar",
                                        );
                                        if ui
                                            .add_enabled(
                                                self.confirm && !blocked,
                                                egui::Button::new(if installed {
                                                    "reinstalar"
                                                } else if step_count > 1 {
                                                    "instalar · ruta completa"
                                                } else {
                                                    "instalar"
                                                }),
                                            )
                                            .clicked()
                                        {
                                            action = Some(entry.clone());
                                            self.confirm = false;
                                        }
                                    });

                                    if let Some(busy) = &self.busy {
                                        ui.separator();
                                        ui.label(
                                            egui::RichText::new(format!("instalando {busy}…"))
                                                .color(theme::SIGNAL),
                                        );
                                        if let Some((end, total)) = self.progress {
                                            let frac = if total == 0 {
                                                0.0
                                            } else {
                                                (end.min(total) as f32) / total as f32
                                            };
                                            ui.add(
                                                egui::ProgressBar::new(frac)
                                                    .show_percentage()
                                                    .desired_width(detail_w - 60.0),
                                            );
                                        }
                                        egui::ScrollArea::vertical()
                                            .id_salt("firmware-log")
                                            .max_height(120.0)
                                            .show(ui, |ui| {
                                                for line in &self.log {
                                                    ui.monospace(line);
                                                }
                                            });
                                    }
                                    if let Some((line, error)) = &self.result {
                                        let color = if *error { theme::HOT } else { theme::SIGNAL };
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(line).color(color),
                                            )
                                            .wrap(),
                                        );
                                    } else if let Some((line, error)) = &self.notice {
                                        let color = if *error { theme::HOT } else { theme::DIM };
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(line).color(color),
                                            )
                                            .wrap(),
                                        );
                                    }
                                });
                        },
                    );
                });
            });

        if refresh {
            self.refresh();
        }
        if backup {
            self.backup();
        }
        if close {
            self.open = false;
        }
        action
    }
}

impl Default for Library {
    fn default() -> Self {
        Self::new(false)
    }
}

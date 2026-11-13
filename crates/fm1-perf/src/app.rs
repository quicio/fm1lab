//! El panel. Mantiene el estado de la sesión (qué paso suena, qué valores se movieron, si hay aparato) y
//! compone las tres zonas. No dibuja: eso está en `panels/`.

use crate::library::Library;
use crate::link::{self, Link, Monitor, Probe, Show, ShowState, Transport};
use crate::panels;
use crate::profiles::{self, strip_label};
use crate::theme::{self, FOOTER_H, HEADER_H, SET_W};
use crate::widgets;
use eframe::egui::{self, Event, Key, Rect, pos2, vec2};
use fm1_core::Scope;
use fm1_core::editor::{self, Clock};
use fm1_core::profile::{Param, ParamAccess, Profile, TransportSpec};
use fm1_core::setlist::{Item, Setlist};
use fm1_store::Panel;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Una franja: un parámetro de un perfil y su valor.
#[derive(Debug, Clone)]
pub struct Strip {
    pub param: Param,
    pub value: i16,
    pub label: String,
}

impl Strip {
    fn frac(&self) -> f32 {
        let span = (self.param.max - self.param.min) as f32;
        if span <= 0.0 {
            return 0.0;
        }
        ((self.value - self.param.min) as f32 / span).clamp(0.0, 1.0)
    }
}

/// Estado del NRPN que llega por el monitor, por canal.
#[derive(Debug, Clone, Copy, Default)]
struct NrpnState {
    msb: u8,
    lsb: u8,
    value: u16,
}

/// La lista buscable para asignar un parámetro del perfil a una franja.
#[derive(Debug, Clone)]
struct Picker {
    target: usize,
    query: String,
    params: Vec<Param>,
    focus: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    /// El set salió del archivo: el sondeo no lo reemplaza.
    File,
    /// El set salió del aparato: se refresca con las lecturas.
    Device,
}

pub struct PerfApp {
    pub setlist: Setlist,
    origin: Origin,
    setlist_path: PathBuf,
    pub current: usize,
    pub project: Option<u8>,
    pub section: char,
    probe: Probe,
    pub link: Link,
    pub profile: Profile,
    profiles: Vec<Profile>,
    jiant: bool,
    panel: Panel,
    panel_path: PathBuf,
    profile_path: PathBuf,
    profile_open: bool,
    picker: Option<Picker>,
    library: Library,
    monitor: Option<Monitor>,
    monitor_open: bool,
    monitor_log: VecDeque<String>,
    learn: bool,
    nrpn: [NrpnState; 16],
    pub tempo: f32,
    pub clock: Clock,
    pub playing: bool,
    transport: Option<Transport>,
    pub focus: usize,
    pub strips: Vec<Strip>,
    pub status: Option<String>,
    pub error: bool,
    pub sent_at: Option<Instant>,
    tempo_from_device: bool,
    auto_close: Option<Instant>,
    auto_started: bool,
    touched: Option<(usize, Instant)>,
    history: Vec<(Instant, String)>,
    demo: bool,
    shot: Option<PathBuf>,
    shot_done: bool,
    shot_started: Instant,
}

impl PerfApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        shot: Option<PathBuf>,
        demo: bool,
        setlist_path: Option<PathBuf>,
        auto_close: Option<u64>,
        profile_override: Option<String>,
    ) -> Self {
        theme::install(&cc.egui_ctx);

        let (profiles, mut problems) = fm1_store::load_profiles();
        let profile_path =
            fm1_store::default_profile_path().unwrap_or_else(|_| PathBuf::from("profile.json"));
        let last = match fm1_store::load_last_profile(&profile_path) {
            Ok(id) => id,
            Err(error) => {
                problems.push(format!("último perfil: {error}"));
                None
            }
        };
        let profile = if demo {
            fm1_core::profile::jiant()
        } else if let Some(id) = profile_override {
            match profiles.iter().find(|profile| profile.id == id) {
                Some(profile) => profile.clone(),
                None => {
                    problems.push(format!("no existe el perfil {id}"));
                    fm1_core::profile::generic()
                }
            }
        } else {
            match link::detect_profile(&profiles) {
                // Si hay aparato manda la identidad. Si no, se recuerda lo último elegido: así el
                // panel no salta a `generic` cada vez que se abre sin el FM-1 conectado.
                Ok(profile) => profile,
                Err(_) => match last
                    .as_deref()
                    .and_then(|id| profiles.iter().find(|profile| profile.id == id).cloned())
                {
                    Some(profile) => profile,
                    None => {
                        if let Some(id) = last {
                            problems.push(format!("el último perfil {id} ya no existe"));
                        }
                        fm1_core::profile::generic()
                    }
                },
            }
        };
        let jiant = profile.id == "jiant-par12";

        let panel_path =
            fm1_store::default_panel_path().unwrap_or_else(|_| PathBuf::from("panel.json"));
        let panel = if demo {
            Panel::new()
        } else {
            match fm1_store::load_panel(&panel_path) {
                Ok(panel) => panel,
                Err(fm1_store::StoreError::Missing(_)) => Panel::new(),
                Err(error) => {
                    problems.push(format!("panel: {error}"));
                    Panel::new()
                }
            }
        };

        let (strips, strip_problems) = profiles::strips_for(&profile, &panel);
        problems.extend(strip_problems);
        let watch = watch_of(&strips);
        let probe = Probe::start(if demo { Vec::new() } else { watch }, jiant && !demo);
        let link = probe.link();
        let monitor = if demo { None } else { Some(Monitor::start()) };

        let (path, path_error) = match setlist_path {
            Some(path) => (path, None),
            None => match fm1_store::default_path() {
                Ok(path) => (path, None),
                Err(error) => (
                    PathBuf::from("setlist.json"),
                    Some(format!("ruta del setlist: {error}")),
                ),
            },
        };

        let (setlist, origin, load_status, load_error) =
            profiles::load_setlist_for(&profile, &path, demo);
        if let Some(error) = path_error {
            problems.push(error);
        }

        let section = setlist
            .items
            .first()
            .and_then(|item| match item {
                Item::Section(section) => Some(*section),
                _ => None,
            })
            .unwrap_or('A');
        let status = if problems.is_empty() {
            load_status
        } else {
            Some(problems.join(" · "))
        };
        let error = !problems.is_empty() || load_error;

        let mut app = Self {
            setlist,
            origin,
            setlist_path: path,
            current: 0,
            project: None,
            section,
            probe,
            link,
            profile,
            profiles,
            jiant,
            panel,
            panel_path,
            profile_path,
            profile_open: false,
            picker: None,
            library: Library::new(demo),
            monitor,
            monitor_open: false,
            monitor_log: VecDeque::new(),
            learn: false,
            nrpn: [NrpnState::default(); 16],
            tempo: 120.0,
            clock: Clock::Int,
            playing: false,
            transport: None,
            focus: 1,
            strips,
            status,
            error,
            sent_at: None,
            tempo_from_device: false,
            auto_close: auto_close.map(|secs| Instant::now() + Duration::from_secs(secs)),
            auto_started: false,
            touched: None,
            history: Vec::new(),
            demo,
            shot,
            shot_done: false,
            shot_started: Instant::now(),
        };
        if demo {
            app.seed_demo();
        }
        app
    }

    /// Estado de ejemplo para capturas: no manda nada al aparato, sólo llena la memoria de la sesión.
    fn seed_demo(&mut self) {
        self.setlist = demo_setlist();
        self.current = 1;
        self.project = Some(3);
        self.section = 'A';
        self.tempo = 140.0;
        self.tempo_from_device = true;
        let now = Instant::now();
        for (seconds, line) in [
            (4_u64, "enviado · sección A"),
            (37, "sección B"),
            (95, "proyecto 3 · B"),
        ] {
            self.history
                .push((now - Duration::from_secs(seconds), line.to_string()));
        }
        self.status = Some("enviado · sección A".to_string());
        self.sent_at = Some(now - Duration::from_secs(4));
        self.touched = Some((1, now - Duration::from_secs(2)));
    }

    // ── estado para el escenario ─────────────────────────────────────────────

    pub fn current_letter(&self) -> char {
        self.section
    }

    pub fn current_project(&self) -> String {
        match self.project {
            Some(n) => format!("proyecto {n}"),
            None => "canción en curso".to_string(),
        }
    }

    pub fn current_subline(&self) -> String {
        let what = match &self.status {
            Some(line) => line.clone(),
            None => "sin enviar".to_string(),
        };
        match self.sent_at {
            Some(at) => format!(
                "{what} · hace {}",
                human(Duration::from_secs(at.elapsed().as_secs()))
            ),
            None => what,
        }
    }

    pub fn strip_view(&self, index: usize) -> (String, String, f32) {
        let strip = &self.strips[index];
        (strip.label.clone(), strip.value.to_string(), strip.frac())
    }

    /// Lo que viene después del paso actual, para el «siguiente» del escenario.
    pub fn next_label(&self) -> Option<String> {
        self.setlist.get(self.current + 1).map(|item| item.label())
    }

    /// Los últimos envíos, para saber qué se mandó y hace cuánto sin mirar el aparato.
    pub fn history_view(&self, count: usize) -> Vec<(String, String)> {
        self.history
            .iter()
            .take(count)
            .map(|(at, line)| {
                (
                    human(Duration::from_secs(at.elapsed().as_secs())),
                    line.clone(),
                )
            })
            .collect()
    }

    /// Si el rango cruza el cero: el relleno sale del centro.
    pub fn is_bipolar(&self, index: usize) -> bool {
        self.strips[index].param.min < 0
    }

    /// Si el valor de esta franja se movió hace poco: eso, y no el foco, es lo que se pinta en señal.
    pub fn is_touched(&self, index: usize) -> bool {
        match self.touched {
            Some((which, at)) => which == index && at.elapsed() < Duration::from_millis(2500),
            None => false,
        }
    }

    // ── acciones ─────────────────────────────────────────────────────────────

    /// Dispara un paso del set y cuenta lo que pasó con el envío.
    ///
    /// Si el transporte estaba parado, primero se carga la sección/proyecto y después se arranca, para
    /// que el primer compás sea el paso disparado. Si ya estaba sonando, el firmware aplica el cambio
    /// al compás siguiente.
    pub fn fire(&mut self, index: usize) {
        if !self.jiant {
            self.fail("este perfil no declara escenas".into());
            return;
        }
        let Some(item) = self.setlist.get(index).cloned() else {
            return;
        };
        self.current = index;
        match &item {
            Item::Section(s) => self.section = s.to_ascii_uppercase(),
            Item::Project { project, section } => {
                self.project = Some(*project);
                self.section = section.to_ascii_uppercase();
            }
            Item::Sound { .. } => {}
        }
        let label = item.label();

        // El firmware sólo carga un proyecto con el transporte parado.
        if self.playing && matches!(item, Item::Project { .. }) {
            self.stop_transport();
        }

        if !self.playing {
            let result = match &item {
                // La carga por el editor confirma la sección; así el primer compás es el pedido y no
                // compite con el arranque del reloj.
                Item::Section(section) => link::load_section(*section),
                _ => link::send(|device| device.play(&item)),
            };
            match result {
                Ok(()) => self.ok(format!("enviado · {label}")),
                Err(error) => {
                    self.fail(format!("falló: {error}"));
                    return;
                }
            }
            self.start_transport();
            return;
        }

        match link::send(|device| device.play(&item)) {
            Ok(()) => self.ok(format!("enviado · {label}")),
            Err(error) => self.fail(format!("falló: {error}")),
        }
    }

    /// Alterna play/stop desde el teclado o el pie.
    pub fn toggle_transport(&mut self) {
        if self.playing {
            match self.stop_transport() {
                Some(error) => self.fail(format!("se cortó el transporte: {error}")),
                None if self.jiant => self.ok("parado · reloj restaurado".into()),
                None => self.ok("parado".into()),
            }
        } else if self.start_transport() {
            if self.jiant {
                self.ok("play · reloj USB".into());
            } else {
                self.ok("play · clock MIDI".into());
            }
        }
    }

    fn start_transport(&mut self) -> bool {
        if self.transport.is_some() {
            self.playing = true;
            return true;
        }
        let started = match self.profile.transport {
            TransportSpec::ClockMaster { clock: Some(_) } => {
                Transport::start_with(self.tempo, self.clock)
            }
            TransportSpec::ClockMaster { clock: None } | TransportSpec::Midi => {
                Transport::start_midi(self.tempo)
            }
            TransportSpec::None => {
                self.fail("el perfil no declara transporte".into());
                return false;
            }
        };
        match started {
            Ok(transport) => {
                self.transport = Some(transport);
                self.playing = true;
                true
            }
            Err(error) => {
                self.fail(format!("play: {error}"));
                false
            }
        }
    }

    fn stop_transport(&mut self) -> Option<String> {
        let error = self.transport.take().and_then(Transport::stop);
        self.playing = false;
        error
    }

    /// Cicla la fuente de reloj del aparato (INT → USB → TRS). Si está sonando, para primero.
    fn cycle_clock(&mut self) {
        if !self.jiant {
            self.fail("este perfil no expone CLOCK".into());
            return;
        }
        if self.playing {
            self.stop_transport();
        }
        let current = link::read_clock().unwrap_or(self.clock);
        let next = match current {
            Clock::Int => Clock::Usb,
            Clock::Usb => Clock::Trs,
            Clock::Trs => Clock::Int,
        };
        match link::set_clock(next) {
            Ok(()) => {
                self.clock = next;
                self.ok(format!("clock = {}", next.name()));
            }
            Err(error) => self.fail(format!("clock: {error}")),
        }
    }

    fn nudge(&mut self, delta: i16) {
        let index = self.focus;
        let param = self.strips[index].param.clone();
        let previous = self.strips[index].value;
        let next = (previous + delta).clamp(param.min, param.max);
        self.strips[index].value = next;
        let name = self.strips[index].label.clone();
        self.touched = Some((index, Instant::now()));
        match link::write_param(&param, next) {
            Ok(()) => self.ok(format!("{name} = {next}")),
            Err(error) => {
                // No se aplicó: la franja vuelve al valor que tenía.
                self.strips[index].value = previous;
                self.fail(format!("{name}: {error}"));
            }
        }
    }

    fn change_tempo(&mut self, delta: f32) {
        let previous = self.tempo;
        self.tempo = (self.tempo + delta).clamp(40.0, 240.0);
        let bpm = self.tempo.round() as i16;
        if self.jiant {
            match link::set_global(editor::BPM_GLOBAL, bpm) {
                Ok(()) => {
                    if let Some(transport) = &self.transport {
                        transport.set_tempo(self.tempo);
                    }
                    self.ok(format!("tempo = {bpm}"));
                }
                Err(error) => {
                    self.tempo = previous;
                    self.fail(format!("tempo: {error}"));
                }
            }
        } else {
            if let Some(transport) = &self.transport {
                transport.set_tempo(self.tempo);
            }
            self.ok(format!("tempo = {bpm}"));
        }
    }

    fn panic(&mut self) {
        self.stop_transport();
        match link::send(|device| device.panic()) {
            Ok(()) => {
                self.playing = false;
                self.ok("pánico · 16 canales".to_string());
            }
            Err(error) => self.fail(format!("pánico: {error}")),
        }
    }

    // ── lectura del aparato ──────────────────────────────────────────────────

    /// Toma el snapshot que dejó el hilo de sondeo, si hay uno nuevo.
    fn take_show(&mut self) {
        if self.demo {
            // Una captura de demo tiene que ser estable: no la pisa lo que haya conectado.
            let _ = self.probe.take_show();
            return;
        }
        let Some(state) = self.probe.take_show() else {
            return;
        };
        match state {
            ShowState::Ready(show) => self.apply_show(show),
            ShowState::Failed(error) => self.fail(format!("lectura: {error}")),
        }
    }

    fn apply_show(&mut self, show: Show) {
        // El tempo se adopta del aparato una sola vez, al arrancar. Después es de la app: si una
        // relectura trae el BPM que el firmware estima de nuestro clock, no puede hacerlo oscilar.
        if !self.tempo_from_device {
            self.tempo = f32::from(show.tempo);
            self.tempo_from_device = true;
        }
        self.clock = show.clock;
        // Un set cargado de archivo manda: el sondeo no lo reemplaza. `r` cambia a `Device`.
        if self.origin != Origin::Device {
            return;
        }
        let set = link::setlist_from_show(&show);
        let steps = set.len();
        self.project = None;
        self.current = 0;
        self.setlist = set;
        self.sent_at = None;
        if let Some(Item::Section(section)) = show.items.first() {
            self.section = *section;
        }
        if steps == 0 {
            self.fail("el proyecto no tiene secciones guardadas".into());
        } else {
            self.status = Some(format!("leído · {steps} pasos"));
            self.error = false;
        }
    }

    // ── seguimiento del aparato ──────────────────────────────────────────────

    fn take_snapshot(&mut self) {
        if self.demo {
            // Una captura de demo no depende de lo que haya conectado.
            let _ = self.probe.take_snapshot();
            return;
        }
        let Some(snapshot) = self.probe.take_snapshot() else {
            return;
        };
        let fresh: Vec<bool> = (0..self.strips.len())
            .map(|index| self.is_fresh(index))
            .collect();
        for (index, strip) in self.strips.iter_mut().enumerate() {
            if fresh[index] {
                continue;
            }
            if let Some(Some(value)) = snapshot.values.get(index) {
                strip.value = (*value).clamp(strip.param.min, strip.param.max);
            }
        }
        let jiant = self.jiant;
        for strip in &mut self.strips {
            strip.label = strip_label(&strip.param, snapshot.selected, jiant);
        }
    }

    /// Si la franja se movió hace muy poco, un sondeo anterior no la pisa.
    fn is_fresh(&self, index: usize) -> bool {
        match self.touched {
            Some((which, at)) => which == index && at.elapsed() < Duration::from_millis(400),
            None => false,
        }
    }

    // ── archivo ──────────────────────────────────────────────────────────────

    fn save_setlist(&mut self) {
        if !self.jiant {
            self.fail("este perfil no usa setlist".into());
            return;
        }
        match fm1_store::save(&self.setlist_path, &self.setlist) {
            Ok(()) => {
                let name = self
                    .setlist_path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| self.setlist_path.display().to_string());
                self.ok(format!("guardado · {} pasos · {name}", self.setlist.len()));
            }
            Err(error) => self.fail(format!("guardar: {error}")),
        }
    }

    fn load_setlist(&mut self) {
        if !self.jiant {
            self.fail("este perfil no usa setlist".into());
            return;
        }
        match fm1_store::load(&self.setlist_path) {
            Ok(set) => {
                let steps = set.len();
                self.setlist = set;
                self.origin = Origin::File;
                self.current = 0;
                self.project = None;
                self.sent_at = None;
                if let Some(Item::Section(section)) = self.setlist.items.first() {
                    self.section = *section;
                }
                self.ok(format!("cargado · {steps} pasos"));
            }
            Err(error) => self.fail(format!("cargar: {error}")),
        }
    }

    // ── franjas asignables ───────────────────────────────────────────────────

    fn open_picker(&mut self) {
        self.picker = Some(Picker {
            target: self.focus,
            query: String::new(),
            params: self.profile.params.clone(),
            focus: true,
        });
    }

    fn watch(&self) -> Vec<Param> {
        self.strips
            .iter()
            .map(|strip| strip.param.clone())
            .collect()
    }

    fn assign(&mut self, index: usize, param: Param) {
        let value = link::read_param(&param)
            .ok()
            .flatten()
            .unwrap_or(param.default);
        let label = strip_label(&param, 0, self.jiant);
        self.strips[index] = Strip {
            value: value.clamp(param.min, param.max),
            label,
            param: param.clone(),
        };
        self.touched = None;
        let entry = self.panel.entry(self.profile.id.clone()).or_default();
        if entry.len() <= index {
            entry.resize(index + 1, String::new());
        }
        entry[index] = param.id.clone();
        let panel = self.panel.clone();
        if let Err(error) = fm1_store::save_panel(&self.panel_path, &panel) {
            self.fail(format!("panel: {error}"));
        } else {
            self.ok(format!("franja {} = {}", index + 1, param.label));
        }
        self.probe.set_watch(self.watch());
    }

    fn draw_picker(&mut self, ctx: &egui::Context) {
        let Some(mut picker) = self.picker.take() else {
            return;
        };
        let title = format!(
            "franja {} · {} · {}",
            picker.target + 1,
            self.profile.name,
            self.strips[picker.target].label
        );
        let mut assign: Option<Param> = None;
        let mut close = false;
        egui::Window::new("asignar parámetro")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([680.0, 520.0])
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(title);
                ui.label("buscá por nombre o id; click para asignar · esc cierra");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut picker.query)
                        .hint_text("buscar…")
                        .desired_width(f32::INFINITY),
                );
                if picker.focus {
                    response.request_focus();
                    picker.focus = false;
                }
                ui.separator();
                let query = picker.query.to_lowercase();
                egui::ScrollArea::vertical()
                    .max_height(400.0)
                    .show(ui, |ui| {
                        for param in &picker.params {
                            if !query.is_empty()
                                && !param.id.to_lowercase().contains(&query)
                                && !param.label.to_lowercase().contains(&query)
                            {
                                continue;
                            }
                            let row = format!(
                                "{:<14} {:<16} {:>4}..{:<4}",
                                param.id, param.label, param.min, param.max
                            );
                            if ui.selectable_label(false, row).clicked() {
                                assign = Some(param.clone());
                            }
                        }
                    });
                if ui.button("cerrar").clicked() {
                    close = true;
                }
            });
        if let Some(param) = assign {
            self.assign(picker.target, param);
        } else if !close {
            self.picker = Some(picker);
        }
    }

    // ── monitor / learn ──────────────────────────────────────────────────────

    fn observe(&mut self, message: &[u8]) {
        let Some((channel, cc, value)) = cc_of(message) else {
            return;
        };
        let index = (channel - 1) as usize;
        let mut state = self.nrpn[index];
        match cc {
            99 => state.msb = value,
            98 => state.lsb = value,
            6 => state.value = (state.value & 0x007F) | ((value as u16) << 7),
            38 => state.value = (state.value & 0x3F80) | value as u16,
            _ => {}
        }
        self.nrpn[index] = state;

        let label = if cc == 6 || cc == 38 {
            format!(
                "NRPN c{channel} {}:{} = {}",
                state.msb, state.lsb, state.value
            )
        } else {
            format!("CC c{channel} {cc} = {value}")
        };
        self.monitor_log
            .push_back(format!("{label}   [{}]", hex_bytes(message)));
        if self.monitor_log.len() > 24 {
            self.monitor_log.pop_front();
        }

        if self.learn {
            if (cc == 6 || cc == 38) && !(state.msb == 127 && state.lsb == 127) {
                let param = learned_nrpn(channel, state.msb, state.lsb, state.value);
                self.learn_param(param);
                return;
            }
            if !matches!(cc, 6 | 38 | 98 | 99 | 100 | 101) {
                let param = learned_cc(channel, cc, value);
                self.learn_param(param);
                return;
            }
        }

        let nrpn = if cc == 6 || cc == 38 {
            Some((state.msb, state.lsb, state.value))
        } else {
            None
        };
        self.apply_monitor(channel, cc, value, nrpn);
    }

    fn apply_monitor(&mut self, channel: u8, cc: u8, value: u8, nrpn: Option<(u8, u8, u16)>) {
        for strip in &mut self.strips {
            match strip.param.access {
                ParamAccess::Cc {
                    channel: want_channel,
                    cc: want_cc,
                } if want_channel == channel && want_cc == cc => {
                    strip.value = (value as i16).clamp(strip.param.min, strip.param.max);
                }
                ParamAccess::Nrpn {
                    channel: want_channel,
                    msb,
                    lsb,
                } => {
                    if let Some((got_msb, got_lsb, raw)) = nrpn
                        && want_channel == channel
                        && got_msb == msb
                        && got_lsb == lsb
                    {
                        strip.value = from_14bit(strip.param.min, strip.param.max, raw);
                    }
                }
                _ => {}
            }
        }
    }

    fn learn_param(&mut self, param: Param) {
        if self.profile.param(&param.id).is_none() {
            self.profile.params.push(param.clone());
        }
        let saved = fm1_store::save_profile(&self.profile);
        let target = self.focus;
        self.assign(target, param.clone());
        self.learn = false;
        match saved {
            Ok(path) => {
                self.status = Some(format!(
                    "aprendido · {} · {}",
                    param.label,
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string())
                ));
                self.error = false;
            }
            Err(error) => self.fail(format!("aprendido, pero no se guardó el perfil: {error}")),
        }
    }

    fn arm_learn(&mut self) {
        self.learn = true;
        self.monitor_open = true;
        self.status = Some("aprender: mové una perilla del aparato".into());
        self.error = false;
    }

    fn draw_monitor(&mut self, ctx: &egui::Context) {
        if !self.monitor_open {
            return;
        }
        let mut close = false;
        let mut learn = false;
        let mut clear = false;
        let learning = self.learn;
        egui::Window::new("monitor MIDI")
            .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
            .fixed_size([560.0, 380.0])
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                if learning {
                    ui.label("esperando CC/NRPN: mové una perilla del aparato…");
                } else {
                    ui.label("últimos mensajes del aparato");
                }
                ui.horizontal(|ui| {
                    if ui.button("aprender").clicked() {
                        learn = true;
                    }
                    if ui.button("limpiar").clicked() {
                        clear = true;
                    }
                    if ui.button("cerrar").clicked() {
                        close = true;
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for line in self.monitor_log.iter().rev() {
                        ui.monospace(line);
                    }
                });
            });
        if learn {
            self.arm_learn();
        }
        if clear {
            self.monitor_log.clear();
        }
        if close {
            self.monitor_open = false;
        }
    }

    /// Si el perfil declara escenas (JIANT). La UI de setlist se muestra sólo entonces.
    pub fn has_scenes(&self) -> bool {
        self.jiant
    }

    /// Abre la biblioteca de firmwares (hook de `--library` y de la tecla `f`).
    pub fn open_library(&mut self) {
        self.library.toggle();
    }

    /// Abre el selector de perfil (hook de `--profiles` y de la tecla `p`).
    pub fn open_profiles(&mut self) {
        self.profile_open = true;
    }

    /// Cambia de perfil en caliente: franjas, setlist, sondeo y persistencia.
    pub fn use_profile(&mut self, profile: Profile, persist: bool) {
        if self.profile.id == profile.id {
            return;
        }
        let (strips, mut problems) = profiles::strips_for(&profile, &self.panel);
        if strips.is_empty() {
            self.fail(format!("{} no declara franjas", profile.name));
            return;
        }
        if self.playing {
            let _ = self.stop_transport();
        }
        let jiant = profile.id == "jiant-par12";
        let (setlist, origin, load_status, load_error) =
            profiles::load_setlist_for(&profile, &self.setlist_path, self.demo);
        let section = setlist
            .items
            .first()
            .and_then(|item| match item {
                Item::Section(section) => Some(*section),
                _ => None,
            })
            .unwrap_or('A');
        let watch = watch_of(&strips);

        self.profile = profile.clone();
        self.jiant = jiant;
        self.strips = strips;
        self.focus = self.focus.min(self.strips.len() - 1);
        self.setlist = setlist;
        self.origin = origin;
        self.current = 0;
        self.project = None;
        self.section = section;
        self.tempo_from_device = false;
        self.sent_at = None;
        self.touched = None;
        self.picker = None;
        self.monitor_log.clear();
        // Lo leído con el perfil anterior no puede aplicarse al nuevo.
        let _ = self.probe.take_show();
        let _ = self.probe.take_snapshot();
        self.probe.set_watch(watch);
        self.probe.set_show(jiant && !self.demo);
        self.probe.refresh();

        let mut line = format!(
            "perfil · {} · {} params",
            profile.name,
            profile.params.len()
        );
        if let Some(loaded) = load_status {
            line = format!("{line} · {loaded}");
        }
        if problems.is_empty() {
            self.status = Some(line);
            self.error = load_error;
        } else {
            problems.insert(0, line);
            self.status = Some(problems.join(" · "));
            self.error = true;
        }
        if persist && let Err(error) = fm1_store::save_last_profile(&self.profile_path, &profile.id)
        {
            self.fail(format!("perfil cambiado, pero no se guardó: {error}"));
        }
    }

    /// La lista de perfiles built-in + usuario, con el actual marcado.
    fn draw_profiles(&mut self, ctx: &egui::Context) {
        if !self.profile_open {
            return;
        }
        match profiles::draw(ctx, &self.profiles, &self.profile.id, &self.link) {
            profiles::Action::Choose(profile) => {
                self.use_profile(profile, true);
                self.profile_open = false;
            }
            profiles::Action::Close => self.profile_open = false,
            profiles::Action::None => {}
        }
    }

    /// El hilo de clock terminó solo (aparato desconectado, envío fallido).
    fn check_transport(&mut self) {
        let Some(error) = self.transport.as_ref().and_then(Transport::error) else {
            return;
        };
        self.transport = None;
        self.playing = false;
        self.fail(format!("se cortó el transporte: {error}"));
        self.probe.refresh();
    }

    fn ok(&mut self, line: String) {
        self.history.insert(0, (Instant::now(), line.clone()));
        self.history.truncate(6);
        self.status = Some(line);
        self.error = false;
        self.sent_at = Some(Instant::now());
    }

    fn fail(&mut self, line: String) {
        self.status = Some(line);
        self.error = true;
    }

    // ── teclado ──────────────────────────────────────────────────────────────

    fn keys(&mut self, ctx: &egui::Context) {
        if self.library.is_open() {
            // La biblioteca es modal: el buscador se lleva las teclas y sólo esc la cierra.
            let events = ctx.input(|input| input.events.clone());
            for event in events {
                if let Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                } = event
                {
                    self.library.close();
                }
            }
            return;
        }
        if self.profile_open {
            // El selector de perfil también es modal.
            let events = ctx.input(|input| input.events.clone());
            for event in events {
                if let Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                } = event
                {
                    self.profile_open = false;
                }
            }
            return;
        }
        if self.picker.is_some() {
            // Con el picker abierto, las teclas van al buscador; sólo esc lo cierra.
            let events = ctx.input(|input| input.events.clone());
            for event in events {
                if let Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                } = event
                {
                    self.picker = None;
                }
            }
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            let Event::Key {
                key,
                pressed: true,
                repeat,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            match key {
                Key::Num1 | Key::Num2 | Key::Num3 | Key::Num4 | Key::Num5 | Key::Num6 => {
                    let index = match key {
                        Key::Num1 => 0,
                        Key::Num2 => 1,
                        Key::Num3 => 2,
                        Key::Num4 => 3,
                        Key::Num5 => 4,
                        _ => 5,
                    };
                    if index < self.setlist.len() {
                        self.fire(index);
                    }
                }
                Key::ArrowUp => self.focus = self.focus.saturating_sub(1),
                Key::ArrowDown => self.focus = (self.focus + 1).min(self.strips.len() - 1),
                Key::ArrowLeft => self.nudge(if modifiers.shift { -8 } else { -1 }),
                Key::ArrowRight => self.nudge(if modifiers.shift { 8 } else { 1 }),
                Key::Plus | Key::Equals => self.change_tempo(if modifiers.alt { 5.0 } else { 1.0 }),
                Key::Minus => self.change_tempo(if modifiers.alt { -5.0 } else { -1.0 }),
                Key::Space if !repeat => self.toggle_transport(),
                Key::Escape => {
                    if self.monitor_open {
                        self.monitor_open = false;
                    } else {
                        self.panic();
                    }
                }
                Key::S if !repeat => self.save_setlist(),
                Key::L if !repeat => self.load_setlist(),
                Key::C if !repeat => self.cycle_clock(),
                Key::A if !repeat => self.open_picker(),
                Key::M if !repeat => self.monitor_open = !self.monitor_open,
                Key::K if !repeat => self.arm_learn(),
                Key::F if !repeat => self.library.toggle(),
                Key::P if !repeat => self.profile_open = !self.profile_open,
                Key::R if !repeat => {
                    if self.jiant {
                        self.origin = Origin::Device;
                    }
                    self.probe.refresh();
                    self.status = Some("releyendo el aparato…".into());
                    self.error = false;
                }
                _ => {}
            }
        }
    }

    // ── captura ──────────────────────────────────────────────────────────────

    fn screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.shot.clone() else {
            return;
        };
        if self.shot_done {
            return;
        }
        // La identidad y el sondeo llegan de hilos: se espera un instante para que la captura
        // muestre el estado real y no “preguntando…”.
        if self.shot_started.elapsed() < Duration::from_millis(1500) {
            ctx.request_repaint();
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            if let Event::Screenshot { image, .. } = event {
                match save_png(&path, &image) {
                    Ok(()) => println!("shot: {}", path.display()),
                    Err(error) => eprintln!("shot: {error}"),
                }
                self.shot_done = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        ctx.request_repaint();
    }
}

impl eframe::App for PerfApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Hook de verificación: arranca el reloj y cierra solo, para probar la restauración del CLOCK.
        if let Some(deadline) = self.auto_close {
            if !self.auto_started {
                self.auto_started = true;
                let _ = self.start_transport();
                eprintln!("fm1-perf: --play-close arrancó el reloj");
            }
            if Instant::now() >= deadline {
                self.auto_close = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        // Cerrar la ventana con el reloj andando tiene que parar y restaurar antes de que eframe salga.
        if ctx.input(|input| input.viewport().close_requested()) && self.playing {
            let _ = self.stop_transport();
        }
        let messages = self.monitor.as_ref().map(Monitor::take).unwrap_or_default();
        for message in messages {
            self.observe(&message);
        }
        self.take_show();
        self.take_snapshot();
        self.check_transport();
        if let Some((line, error)) = self.library.poll() {
            self.status = Some(line);
            self.error = error;
            self.sent_at = Some(Instant::now());
        }
        // El flash terminó: el sondeo y el monitor vuelven a tocar el aparato.
        if !self.library.is_busy() {
            if self.probe.is_paused() {
                self.probe.resume();
                self.probe.refresh();
            }
            if let Some(monitor) = &self.monitor
                && monitor.is_paused()
            {
                monitor.resume();
            }
        }
        if self.library.take_flash_completion() {
            // El firmware nuevo ya está corriendo: se re-detecta sin reiniciar la app.
            match link::detect_profile(&self.profiles) {
                Ok(profile) => {
                    let name = profile.name.clone();
                    self.use_profile(profile, true);
                    self.status = Some(format!("instalado · perfil {name}"));
                    self.error = false;
                }
                Err(_) => {
                    self.status = Some("instalado · reiniciá fm1lab si el perfil no cambió".into());
                    self.error = false;
                }
            }
        }
        self.keys(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme::INK))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let header = Rect::from_min_size(rect.min, vec2(rect.width(), HEADER_H));
                let ticks = Rect::from_min_size(
                    pos2(rect.left(), header.bottom()),
                    vec2(rect.width(), 7.0),
                );
                let footer = Rect::from_min_size(
                    pos2(rect.left(), rect.bottom() - FOOTER_H),
                    vec2(rect.width(), FOOTER_H),
                );
                let body = Rect::from_min_max(
                    pos2(rect.left(), ticks.bottom()),
                    pos2(rect.right(), footer.top()),
                );
                let set_w = if self.has_scenes() { SET_W } else { 0.0 };
                let set = Rect::from_min_size(body.min, vec2(set_w, body.height()));
                let stage = Rect::from_min_max(pos2(set.right(), body.top()), body.max);

                panels::chrome::header(self, ui, header);
                widgets::hline(ui, rect.left(), rect.right(), header.bottom(), theme::LINE);
                panels::chrome::ticks(ui, ticks);
                panels::chrome::hline_below(ui, ticks);

                if self.has_scenes() {
                    widgets::vline(ui, set.right(), body.top(), body.bottom(), theme::LINE);
                    panels::setlist::draw(self, ui, set);
                }
                panels::stage::draw(self, ui, stage);

                widgets::hline(ui, rect.left(), rect.right(), footer.top(), theme::LINE);
                panels::chrome::footer(self, ui, footer);
            });

        self.draw_picker(ctx);
        self.draw_monitor(ctx);
        self.draw_profiles(ctx);
        if let Some(entry) = self.library.draw(ctx) {
            // Un OTA no convive con clock, sondeo ni monitor: primero se sueltan todos.
            if self.playing {
                let _ = self.stop_transport();
            }
            self.monitor_open = false;
            self.probe.pause();
            if let Some(monitor) = &self.monitor {
                monitor.pause();
            }
            self.status = Some(format!("instalando {}…", entry.name));
            self.error = false;
            self.library.start_install(entry);
        }
        self.link = self.probe.link();
        self.screenshot(ctx);
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    /// eframe puede salir sin correr los destructores: acá se para el reloj y se restaura el CLOCK.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.transport.take().and_then(Transport::stop);
        self.playing = false;
    }
}

fn cc_of(message: &[u8]) -> Option<(u8, u8, u8)> {
    if message.len() >= 3 && (message[0] & 0xF0) == 0xB0 {
        Some((
            (message[0] & 0x0F) + 1,
            message[1] & 0x7F,
            message[2] & 0x7F,
        ))
    } else {
        None
    }
}

fn hex_bytes(message: &[u8]) -> String {
    message
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn from_14bit(min: i16, max: i16, raw: u16) -> i16 {
    let span = (max as i32 - min as i32).max(1);
    (min as i32 + (raw as i32 * span + 8191) / 16383).clamp(min as i32, max as i32) as i16
}

fn learned_cc(channel: u8, cc: u8, value: u8) -> Param {
    Param::new(
        format!("cc.{channel}.{cc}"),
        format!("CC {channel}.{cc}"),
        Scope::Track,
        0,
        127,
        value as i16,
        ParamAccess::Cc { channel, cc },
    )
}

fn learned_nrpn(channel: u8, msb: u8, lsb: u8, value: u16) -> Param {
    Param::new(
        format!("nrpn.{channel}.{msb}.{lsb}"),
        format!("NRPN {channel}:{msb}:{lsb}"),
        Scope::Track,
        0,
        16383,
        value as i16,
        ParamAccess::Nrpn { channel, msb, lsb },
    )
}

fn watch_of(strips: &[Strip]) -> Vec<Param> {
    strips.iter().map(|strip| strip.param.clone()).collect()
}

pub(crate) fn demo_setlist() -> Setlist {
    let mut set = Setlist::new("viernes");
    for item in [
        Item::Project {
            project: 3,
            section: 'B',
        },
        Item::Section('A'),
        Item::Section('B'),
        Item::Sound {
            track: 1,
            program: 41,
        },
        Item::Project {
            project: 2,
            section: 'D',
        },
        Item::Section('C'),
    ] {
        let _ = set.push(item);
    }
    set
}

fn human(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{secs} s")
    } else {
        format!("{} min", secs / 60)
    }
}

fn save_png(path: &std::path::Path, image: &egui::ColorImage) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let writer = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, image.width() as u32, image.height() as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    let mut bytes = Vec::with_capacity(image.pixels.len() * 4);
    for pixel in &image.pixels {
        bytes.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b(), pixel.a()]);
    }
    writer.write_image_data(&bytes).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_monitor_lee_cc_y_hex() {
        assert_eq!(cc_of(&[0xB0, 7, 100]), Some((1, 7, 100)));
        assert_eq!(cc_of(&[0xB3, 74, 12]), Some((4, 74, 12)));
        assert_eq!(cc_of(&[0x90, 60, 100]), None);
        assert_eq!(hex_bytes(&[0xB0, 7, 100]), "B0 07 64");
    }

    #[test]
    fn el_valor_nrpn_vuelve_al_rango_del_parametro() {
        assert_eq!(from_14bit(0, 127, 0), 0);
        assert_eq!(from_14bit(0, 127, 16383), 127);
        assert_eq!(from_14bit(-64, 63, 8192), 0);
        assert_eq!(from_14bit(0, 16383, 1234), 1234);
    }

    #[test]
    fn lo_aprendido_arma_un_parametro_coherente() {
        let cc = learned_cc(2, 74, 12);
        assert_eq!(cc.id, "cc.2.74");
        assert_eq!(cc.access, ParamAccess::Cc { channel: 2, cc: 74 });
        assert_eq!(cc.default, 12);

        let nrpn = learned_nrpn(1, 0, 5, 9000);
        assert_eq!(nrpn.id, "nrpn.1.0.5");
        assert_eq!(
            nrpn.access,
            ParamAccess::Nrpn {
                channel: 1,
                msb: 0,
                lsb: 5
            }
        );
        assert_eq!(nrpn.max, 16383);
        assert_eq!(nrpn.default, 9000);
    }

    #[test]
    fn las_franjas_respetan_el_panel_y_se_completan_con_el_perfil() {
        let profile = fm1_core::profile::x0x();
        let mut panel = Panel::new();
        panel.insert(
            "x0x-1.2".into(),
            vec!["303a.cutoff".into(), "909.bd.level".into()],
        );
        let (strips, problems) = profiles::strips_for(&profile, &panel);
        assert!(problems.is_empty());
        assert_eq!(strips.len(), 4);
        assert_eq!(strips[0].param.id, "303a.cutoff");
        assert_eq!(strips[1].param.id, "909.bd.level");
        // El resto sale de `default_strips` sin repetir.
        assert_eq!(strips[2].param.id, "909.level");
        assert_eq!(strips[3].param.id, "909.pan");
    }

    #[test]
    fn un_perfil_que_no_usa_setlist_arranca_vacio() {
        let path = std::env::temp_dir().join("fm1-perf-setlist-inexistente.json");
        let (set, origin, status, error) =
            profiles::load_setlist_for(&fm1_core::profile::x0x(), &path, false);
        assert_eq!(set.len(), 0);
        assert_eq!(origin, Origin::File);
        assert!(status.is_none());
        assert!(!error);
    }
}

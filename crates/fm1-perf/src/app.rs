//! El panel. Mantiene el estado de la sesión (qué paso suena, qué valores se movieron, si hay aparato) y
//! compone las tres zonas. No dibuja: eso está en `panels/`.

use crate::link::{self, Link, Probe};
use crate::panels;
use crate::theme::{self, FOOTER_H, HEADER_H, SET_W};
use crate::widgets;
use eframe::egui::{self, Event, Key, Rect, pos2, vec2};
use fm1_core::setlist::{Item, Setlist};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// De dónde sale un parámetro de franja: del mapa generado, nunca de un número escrito acá.
#[derive(Debug, Clone, Copy)]
pub enum StripKind {
    /// Pista 1, parámetro `FILTER` (id 5 del mapa, −64..63).
    TrackFilter(u8),
    /// Un global por id (BPM es 0, DJ FILT 17, MIX 7, SIZE 8).
    GlobalId(u8),
}

#[derive(Debug, Clone)]
pub struct Strip {
    pub label: &'static str,
    pub kind: StripKind,
    pub value: i16,
    pub min: i16,
    pub max: i16,
}

impl Strip {
    fn frac(&self) -> f32 {
        let span = (self.max - self.min) as f32;
        if span <= 0.0 {
            return 0.0;
        }
        ((self.value - self.min) as f32 / span).clamp(0.0, 1.0)
    }

    fn clamp(&mut self, next: i16) {
        self.value = next.clamp(self.min, self.max);
    }
}

pub struct PerfApp {
    pub setlist: Setlist,
    pub current: usize,
    pub project: Option<u8>,
    pub section: char,
    probe: Probe,
    pub link: Link,
    pub tempo: f32,
    pub playing: bool,
    pub focus: usize,
    pub strips: Vec<Strip>,
    pub status: Option<String>,
    pub error: bool,
    pub sent_at: Option<Instant>,
    touched: Option<(usize, Instant)>,
    history: Vec<(Instant, String)>,
    shot: Option<PathBuf>,
    shot_done: bool,
}

impl PerfApp {
    pub fn new(cc: &eframe::CreationContext<'_>, shot: Option<PathBuf>, demo: bool) -> Self {
        theme::install(&cc.egui_ctx);
        let probe = Probe::start();
        let link = probe.link();
        let mut app = Self {
            setlist: demo_setlist(),
            current: 1,
            project: Some(3),
            section: 'A',
            probe,
            link,
            tempo: 140.0,
            playing: true,
            focus: 1,
            strips: vec![
                Strip {
                    label: "cabezal t1",
                    kind: StripKind::TrackFilter(1),
                    value: 0,
                    min: -64,
                    max: 63,
                },
                Strip {
                    label: "dj filter",
                    kind: StripKind::GlobalId(17),
                    value: 0,
                    min: -64,
                    max: 63,
                },
                Strip {
                    label: "reverb size",
                    kind: StripKind::GlobalId(8),
                    value: 90,
                    min: 0,
                    max: 127,
                },
                Strip {
                    label: "mix",
                    kind: StripKind::GlobalId(7),
                    value: 90,
                    min: 0,
                    max: 127,
                },
            ],
            status: None,
            error: false,
            sent_at: None,
            touched: None,
            history: Vec::new(),
            shot,
            shot_done: false,
        };
        if demo {
            app.seed_demo();
        }
        app
    }

    /// Estado de ejemplo para capturas: no manda nada al aparato, sólo llena la memoria de la sesión.
    fn seed_demo(&mut self) {
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
        (
            strip.label.to_string(),
            strip.value.to_string(),
            strip.frac(),
        )
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
        self.strips[index].min < 0
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
    pub fn fire(&mut self, index: usize) {
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
        match link::send(|device| device.play(&item)) {
            Ok(()) => self.ok(format!("enviado · {label}")),
            Err(error) => self.fail(format!("falló: {error}")),
        }
    }

    fn nudge(&mut self, delta: i16) {
        let index = self.focus;
        let kind = self.strips[index].kind;
        let next = self.strips[index].value + delta;
        self.strips[index].clamp(next);
        let value = self.strips[index].value;
        let name = self.strips[index].label;
        self.touched = Some((index, Instant::now()));
        let result = match kind {
            StripKind::TrackFilter(track) => {
                link::send(|device| device.set_track(track, "FILTER", value))
            }
            StripKind::GlobalId(id) => link::send(|device| device.set_global_id(id, value)),
        };
        match result {
            Ok(()) => self.ok(format!("{name} = {value}")),
            Err(error) => self.fail(format!("{name}: {error}")),
        }
    }

    fn change_tempo(&mut self, delta: f32) {
        self.tempo = (self.tempo + delta).clamp(40.0, 240.0);
        let bpm = self.tempo.round() as i16;
        match link::send(|device| device.set_bpm(bpm)) {
            Ok(()) => self.ok(format!("tempo = {bpm}")),
            Err(error) => self.fail(format!("tempo: {error}")),
        }
    }

    fn panic(&mut self) {
        match link::send(|device| device.panic()) {
            Ok(()) => {
                self.playing = false;
                self.ok("pánico · 16 canales".to_string());
            }
            Err(error) => self.fail(format!("pánico: {error}")),
        }
    }

    fn ok(&mut self, line: String) {
        self.history.insert(0, (Instant::now(), line.clone()));
        self.history.truncate(6);
        self.status = Some(line);
        self.error = false;
        self.sent_at = Some(Instant::now());
        self.playing = true;
    }

    fn fail(&mut self, line: String) {
        self.status = Some(line);
        self.error = true;
    }

    // ── teclado ──────────────────────────────────────────────────────────────

    fn keys(&mut self, ctx: &egui::Context) {
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
                Key::ArrowLeft => {
                    self.focus = self.focus.saturating_sub(1);
                }
                Key::ArrowRight => self.focus = (self.focus + 1).min(self.strips.len() - 1),
                Key::ArrowUp => self.nudge(if modifiers.shift { 8 } else { 1 }),
                Key::ArrowDown => self.nudge(if modifiers.shift { -8 } else { -1 }),
                Key::Plus | Key::Equals => self.change_tempo(if modifiers.alt { 5.0 } else { 1.0 }),
                Key::Minus => self.change_tempo(if modifiers.alt { -5.0 } else { -1.0 }),
                Key::Escape => self.panic(),
                Key::R if !repeat => {
                    self.probe = Probe::start();
                    self.link = self.probe.link();
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
                let set = Rect::from_min_size(body.min, vec2(SET_W, body.height()));
                let stage = Rect::from_min_max(pos2(set.right(), body.top()), body.max);

                panels::chrome::header(self, ui, header);
                widgets::hline(ui, rect.left(), rect.right(), header.bottom(), theme::LINE);
                panels::chrome::ticks(ui, ticks);
                panels::chrome::hline_below(ui, ticks);

                widgets::vline(ui, set.right(), body.top(), body.bottom(), theme::LINE);
                panels::setlist::draw(self, ui, set);
                panels::stage::draw(self, ui, stage);

                widgets::hline(ui, rect.left(), rect.right(), footer.top(), theme::LINE);
                panels::chrome::footer(self, ui, footer);
            });

        self.link = self.probe.link();
        self.screenshot(ctx);
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

fn demo_setlist() -> Setlist {
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

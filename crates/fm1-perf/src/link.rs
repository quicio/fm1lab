//! El enlace con el aparato: descubrir el puerto, leer lo que expone y mandar intenciones.
//!
//! Es el único lugar de la app que habla con `fm1-midi`. La UI no ve bytes ni puertos: pide
//! intenciones, asigna parámetros de un perfil y lee snapshots ya armados. Abrir por comando cuesta
//! milisegundos y evita el problema de un puerto que queda tomado cuando el aparato se desconecta
//! (`midir` no tiene hot-plug).
//!
//! Dos cosas viven en hilos aparte: el sondeo/lectura (`Probe`) y el reloj (`Transport`). Ninguna de
//! las dos bloquea el hilo de dibujo.

use fm1_core::device::Device;
use fm1_core::editor::{self, Clock, SongChain};
use fm1_core::profile::{self, Param, Profile};
use fm1_core::setlist::{Item, Setlist};
use fm1_core::transport::{MidiIn, MidiOut};
use fm1_core::{Error, Result, Scope};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Nombres que publican los firmwares abiertos, en minúsculas.
const KNOWN: [&str; 5] = ["fm-1", "fm1", "jiant", "felucca", "smk"];

/// Cuánto esperar cada respuesta del editor protocol.
const QUERY_WAIT: Duration = fm1_midi::QUERY_WAIT;

/// El reloj maestro vive en el adaptador (es hilo y CoreMIDI, no dominio).
pub type Transport = fm1_midi::ClockMaster;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// Hay puerto: se muestra su nombre.
    Ready(String),
    /// No hay aparato: la lista se edita igual, pero nada se envía.
    Absent,
}

/// Lo que un firmware de la familia JIANT tiene cargado: lo usa el perfil `jiant-par12`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Show {
    /// Las secciones con música, en el orden de la cadena (o A..D si no hay cadena).
    pub items: Vec<Item>,
    /// BPM real del aparato, para arrancar el reloj con el mismo tempo.
    pub tempo: i16,
    /// La fuente de reloj que tenía el aparato al leer.
    pub clock: Clock,
    /// La cadena del song tal como está configurada (puede tener filas vacías).
    pub chain: SongChain,
    /// La pista seleccionada en el aparato (0..5).
    pub selected: u8,
    pub filter: i16,
    pub dj_filter: i16,
    pub size: i16,
    pub mix: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShowState {
    Ready(Show),
    Failed(String),
}

/// Los valores de las franjas asignadas, leídos del aparato.
///
/// `None` en una posición significa "ese protocolo no tiene lectura" (CC/NRPN): la franja muestra el
/// último valor escrito, o el que capture el monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub selected: u8,
    pub values: Vec<Option<i16>>,
}

/// Sondeo de puertos, lectura del set y de las franjas asignadas, en un hilo aparte.
#[derive(Debug, Clone)]
pub struct Probe {
    state: Arc<Mutex<Link>>,
    show: Arc<Mutex<Option<ShowState>>>,
    watch: Arc<Mutex<Vec<Param>>>,
    snapshot: Arc<Mutex<Option<Snapshot>>>,
    refresh: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    wants_show: Arc<AtomicBool>,
}

impl Probe {
    /// `watch` es la lista de parámetros asignados; `wants_show` sólo para perfiles que exponen
    /// secciones/proyectos/cadena (JIANT).
    pub fn start(watch: Vec<Param>, wants_show: bool) -> Self {
        let state = Arc::new(Mutex::new(Link::Absent));
        let show = Arc::new(Mutex::new(None));
        let shared_watch = Arc::new(Mutex::new(watch));
        let snapshot = Arc::new(Mutex::new(None));
        let refresh = Arc::new(AtomicBool::new(false));
        let paused = Arc::new(AtomicBool::new(false));
        let wants_show = Arc::new(AtomicBool::new(wants_show));

        let shared_state = Arc::clone(&state);
        let shared_show = Arc::clone(&show);
        let thread_watch = Arc::clone(&shared_watch);
        let shared_snapshot = Arc::clone(&snapshot);
        let shared_refresh = Arc::clone(&refresh);
        let shared_paused = Arc::clone(&paused);
        let thread_show = Arc::clone(&wants_show);
        thread::spawn(move || {
            let mut first = true;
            let mut was_ready = false;
            let mut ready = false;
            let mut tick = 0_u32;
            loop {
                // Durante un flash nadie más toca el aparato: ni GETs ni handshakes.
                if shared_paused.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(300));
                    continue;
                }
                // Descubrir el puerto cada 1,5 s (3 vueltas de 500 ms).
                if tick.is_multiple_of(3) {
                    let found = fm1_midi::list()
                        .ok()
                        .and_then(|(_, outs)| outs.into_iter().find(|name| looks_like_device(name)))
                        .map(Link::Ready)
                        .unwrap_or(Link::Absent);
                    if let Ok(mut slot) = shared_state.lock()
                        && *slot != found
                    {
                        *slot = found.clone();
                    }

                    ready = matches!(found, Link::Ready(_));
                    let wanted = first
                        || shared_refresh.swap(false, Ordering::AcqRel)
                        || (ready && !was_ready);
                    if wanted && thread_show.load(Ordering::Acquire) {
                        let snapshot = if ready {
                            match read_show() {
                                Ok(show) => ShowState::Ready(show),
                                Err(error) => ShowState::Failed(error.to_string()),
                            }
                        } else {
                            ShowState::Failed("sin aparato".into())
                        };
                        if let Ok(mut slot) = shared_show.lock() {
                            *slot = Some(snapshot);
                        }
                    }
                    if wanted {
                        first = false;
                    }
                    was_ready = ready;
                }

                // Seguimiento: sólo si hay parámetros legibles (editor). CC/NRPN los llena el monitor.
                if ready {
                    let watch = thread_watch
                        .lock()
                        .map(|watch| watch.clone())
                        .unwrap_or_default();
                    if watch.iter().any(|param| param.access.is_readable())
                        && let Ok(snapshot) = read_watch(&watch)
                        && let Ok(mut slot) = shared_snapshot.lock()
                    {
                        *slot = Some(snapshot);
                    }
                }

                tick = tick.wrapping_add(1);
                thread::sleep(Duration::from_millis(500));
            }
        });
        Self {
            state,
            show,
            watch: shared_watch,
            snapshot,
            refresh,
            paused,
            wants_show,
        }
    }

    pub fn link(&self) -> Link {
        self.state.lock().map(|g| g.clone()).unwrap_or(Link::Absent)
    }

    /// Suelta el aparato mientras el instalador lo escribe.
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    /// Cambia si el perfil actual expone secciones/proyectos/cadena (JIANT).
    pub fn set_show(&self, wants: bool) {
        self.wants_show.store(wants, Ordering::Release);
        if wants {
            self.refresh();
        }
    }

    /// El último snapshot del set, una sola vez.
    pub fn take_show(&self) -> Option<ShowState> {
        self.show.lock().ok().and_then(|mut slot| slot.take())
    }

    /// La última foto de las franjas, una sola vez.
    pub fn take_snapshot(&self) -> Option<Snapshot> {
        self.snapshot.lock().ok().and_then(|mut slot| slot.take())
    }

    /// Cambia los parámetros que se siguen al asignar una franja.
    pub fn set_watch(&self, watch: Vec<Param>) {
        if let Ok(mut slot) = self.watch.lock() {
            *slot = watch;
        }
    }

    /// Pide releer el aparato (tecla `r`).
    pub fn refresh(&self) {
        self.refresh.store(true, Ordering::Release);
    }
}

/// Escucha el MIDI de entrada del aparato en un hilo aparte y guarda los últimos mensajes.
///
/// Es la base del monitor/learn: permite ver qué manda un firmware desconocido y capturar el CC/NRPN
/// que el usuario mueve. No manda nada. Se pausa durante un flash: suelta el puerto de entrada.
#[derive(Debug, Clone)]
pub struct Monitor {
    messages: Arc<Mutex<VecDeque<Vec<u8>>>>,
    paused: Arc<AtomicBool>,
}

impl Monitor {
    pub fn start() -> Self {
        let messages = Arc::new(Mutex::new(VecDeque::with_capacity(64)));
        let paused = Arc::new(AtomicBool::new(false));
        let shared = Arc::clone(&messages);
        let shared_paused = Arc::clone(&paused);
        thread::spawn(move || {
            loop {
                if shared_paused.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(400));
                    continue;
                }
                let Ok(mut port) = fm1_midi::Port::open() else {
                    thread::sleep(Duration::from_millis(1000));
                    continue;
                };
                let mut idle = 0_u32;
                loop {
                    // En pausa se suelta el puerto: durante un OTA nadie más lo toca.
                    if shared_paused.load(Ordering::Acquire) {
                        break;
                    }
                    match port.recv(Duration::from_millis(500)) {
                        Some(message) => {
                            idle = 0;
                            if let Ok(mut log) = shared.lock() {
                                if log.len() >= 64 {
                                    log.pop_front();
                                }
                                log.push_back(message);
                            }
                        }
                        None => {
                            idle += 1;
                            if idle >= 60 {
                                // 30 s sin nada: si el puerto ya no está, se reabre arriba.
                                let present = fm1_midi::list()
                                    .ok()
                                    .and_then(|(ins, _)| {
                                        ins.into_iter().find(|name| looks_like_device(name))
                                    })
                                    .is_some();
                                if !present {
                                    break;
                                }
                                idle = 0;
                            }
                        }
                    }
                }
            }
        });
        Self { messages, paused }
    }

    /// Suelta el puerto de entrada mientras el instalador usa el aparato.
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    /// Los mensajes llegados desde la última vez.
    pub fn take(&self) -> Vec<Vec<u8>> {
        self.messages
            .lock()
            .map(|mut log| log.drain(..).collect())
            .unwrap_or_default()
    }
}

/// Detecta el perfil por identidad OTA y versión del editor; cae al genérico si no reconoce.
pub fn detect_profile(profiles: &[Profile]) -> Result<Profile> {
    fm1_midi::detect_profile(profiles)
}

/// La lista de parámetros que trae el perfil JIANT por defecto; la usa el hook `--mix`.
pub fn default_watch() -> Vec<Param> {
    let profile = profile::jiant();
    profile
        .default_strips
        .iter()
        .filter_map(|id| profile.param(id).cloned())
        .collect()
}

/// Lee los valores de una lista de parámetros asignados.
pub fn read_watch(watch: &[Param]) -> Result<Snapshot> {
    let mut port = fm1_midi::Port::open()?;
    let values = port.watch(watch)?;
    let selected = port
        .editor(&editor::track_query(), QUERY_WAIT)
        .and_then(|reply| reply.selected_track())
        .unwrap_or(0);
    Ok(Snapshot { selected, values })
}

/// Lee las cuatro secciones del proyecto cargado, la cadena y el BPM (perfil JIANT).
pub fn read_show() -> Result<Show> {
    let mut port = fm1_midi::Port::open()?;

    let mut used = [false; 4];
    for (slot, value) in used.iter_mut().enumerate() {
        let slot = slot as u8;
        let reply = port
            .editor(&editor::project_query(slot), QUERY_WAIT)
            .ok_or_else(|| Error::Transport("el proyecto no contestó".into()))?;
        *value = reply
            .project_used()
            .filter(|(answered, _)| *answered == slot)
            .is_some_and(|(_, used)| used);
    }

    let chain = port
        .editor(&editor::song_query(), QUERY_WAIT)
        .and_then(|reply| reply.song())
        .ok_or_else(|| Error::Transport("la cadena de song no contestó".into()))?;

    let tempo = port
        .editor(&editor::get(Scope::Global, editor::BPM_GLOBAL), QUERY_WAIT)
        .and_then(|reply| reply.value().map(|(_, value)| value))
        .filter(|value| (40..=240).contains(value))
        .unwrap_or(120);
    let clock = port
        .editor(&editor::clock_get(), QUERY_WAIT)
        .and_then(|reply| reply.value())
        .and_then(|(_, value)| Clock::from_value(value))
        .unwrap_or(Clock::Int);

    let selected = port
        .editor(&editor::track_query(), QUERY_WAIT)
        .and_then(|reply| reply.selected_track())
        .unwrap_or(0);
    let filter = read_value(&mut port, Scope::Track, 5).unwrap_or(0);
    let dj_filter = read_value(&mut port, Scope::Global, 17).unwrap_or(0);
    let size = read_value(&mut port, Scope::Global, 8).unwrap_or(90);
    let mix = read_value(&mut port, Scope::Global, 7).unwrap_or(90);

    let mut items = Vec::new();
    if chain.rows.is_empty() {
        for (slot, present) in used.iter().enumerate() {
            if *present {
                items.push(Item::Section((b'A' + slot as u8) as char));
            }
        }
    } else {
        for row in &chain.rows {
            let slot = row.section as u8;
            if slot.is_ascii_uppercase() && used[(slot - b'A') as usize] {
                items.push(Item::Section(row.section));
            }
        }
    }

    Ok(Show {
        items,
        tempo,
        clock,
        chain,
        selected,
        filter,
        dj_filter,
        size,
        mix,
    })
}

fn read_value(port: &mut fm1_midi::Port, scope: Scope, id: u8) -> Option<i16> {
    port.editor(&editor::get(scope, id), QUERY_WAIT)
        .and_then(|reply| reply.value())
        .filter(|(answered, _)| *answered == id)
        .map(|(_, value)| value)
}

/// Carga una sección con el `PROJECT op 0` del editor y espera la confirmación.
pub fn load_section(section: char) -> Result<()> {
    let slot = editor::section_slot(section)?;
    let mut port = fm1_midi::Port::open()?;
    let reply = port
        .editor(&editor::project_load(slot), QUERY_WAIT)
        .ok_or_else(|| Error::Transport("la sección no contestó al cargar".into()))?;
    let (op, answered, used) = reply
        .project()
        .ok_or_else(|| Error::Transport("respuesta inesperada al cargar".into()))?;
    if op != 0 || answered != slot {
        return Err(Error::Transport(
            "la sección que contestó no es la pedida".into(),
        ));
    }
    if !used {
        return Err(Error::Param(format!(
            "la sección {} está vacía",
            section.to_ascii_uppercase()
        )));
    }
    Ok(())
}

fn looks_like_device(name: &str) -> bool {
    let lower = name.to_lowercase();
    KNOWN.iter().any(|known| lower.contains(known))
}

/// Manda una intención al aparato. Sin aparato devuelve el error del adaptador (no inventa un `ok`).
pub fn send<F>(intent: F) -> Result<()>
where
    F: FnOnce(&mut Device<Box<dyn MidiOut>>) -> Result<()>,
{
    let port = fm1_midi::Port::open()?;
    let mut device = Device::new(Box::new(port) as Box<dyn MidiOut>);
    intent(&mut device)
}

/// Escribe un global por el editor y **espera la confirmación** del aparato.
pub fn set_global(id: u8, value: i16) -> Result<()> {
    set_value(Scope::Global, id, value)
}

/// Lee un parámetro de un perfil por su acceso (`None` si no tiene lectura).
pub fn read_param(param: &Param) -> Result<Option<i16>> {
    let mut port = fm1_midi::Port::open()?;
    port.read_param(param)
}

/// Escribe un parámetro de un perfil por su acceso.
pub fn write_param(param: &Param, value: i16) -> Result<()> {
    let mut port = fm1_midi::Port::open()?;
    port.write_param(param, value)
}

/// Lee la fuente de reloj actual.
pub fn read_clock() -> Result<Clock> {
    let mut port = fm1_midi::Port::open()?;
    let reply = port
        .editor(&editor::clock_get(), QUERY_WAIT)
        .ok_or_else(|| Error::Transport("el reloj no contestó".into()))?;
    let (_, value) = reply
        .value()
        .ok_or_else(|| Error::Transport("respuesta inesperada al leer el reloj".into()))?;
    Clock::from_value(value).ok_or_else(|| Error::Transport("reloj desconocido".into()))
}

/// Pone la fuente de reloj y espera la confirmación.
pub fn set_clock(clock: Clock) -> Result<()> {
    let mut port = fm1_midi::Port::open()?;
    let reply = port
        .editor(&editor::clock_set(clock), QUERY_WAIT)
        .ok_or_else(|| Error::Transport("el aparato no aceptó el reloj".into()))?;
    let (_, value) = reply
        .value()
        .ok_or_else(|| Error::Transport("respuesta inesperada al poner el reloj".into()))?;
    if Clock::from_value(value) != Some(clock) {
        return Err(Error::Transport(format!(
            "el aparato quedó en {}",
            Clock::from_value(value).map(Clock::name).unwrap_or("?")
        )));
    }
    Ok(())
}

fn set_value(scope: Scope, id: u8, value: i16) -> Result<()> {
    let mut port = fm1_midi::Port::open()?;
    let request = editor::set(scope, id, value);
    let reply = port
        .editor(&request, QUERY_WAIT)
        .ok_or_else(|| Error::Transport(format!("el parámetro {id} no confirmó")))?;
    let (answered, applied) = reply
        .value()
        .ok_or_else(|| Error::Transport("respuesta inesperada al escribir".into()))?;
    if answered != id {
        return Err(Error::Transport(
            "el aparato confirmó otro parámetro".into(),
        ));
    }
    if applied != value {
        return Err(Error::Param(format!(
            "el aparato aplicó {applied} en vez de {value}"
        )));
    }
    Ok(())
}

/// Arma el set que muestra la app a partir de un `Show` leído del aparato.
pub fn setlist_from_show(show: &Show) -> Setlist {
    let mut set = Setlist::new("aparato");
    for item in &show.items {
        let _ = set.push(item.clone());
    }
    set
}

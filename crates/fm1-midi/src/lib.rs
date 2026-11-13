//! `fm1-midi` — el adaptador de `midir` para los puertos que declara `fm1-core`.
//!
//! Es la única capa que conoce CoreMIDI, los nombres de los puertos y el hilo de lectura. El dominio no
//! se entera: recibe un `MidiOut` y un `MidiIn` y sigue.

use fm1_core::profile::{Param, ParamAccess, Profile};
use fm1_core::transport::{MidiIn, MidiOut};
use fm1_core::{Error, Result, Scope, editor, message};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Cuánto esperar cada respuesta del editor protocol.
pub const QUERY_WAIT: Duration = Duration::from_millis(350);

/// Los cuatro valores que muestra el panel y la pista que el aparato tiene seleccionada.
///
/// Son lecturas puntuales por el editor protocol: el firmware no manda el eco de NRPN que documenta
/// `docs/NRPN.md`, así que seguirlo es preguntar, no escuchar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mix {
    pub selected: u8,
    pub filter: i16,
    pub dj_filter: i16,
    pub size: i16,
    pub mix: i16,
}

/// Ids del mapa para las cuatro franjas: FILTER de pista, DJ FILT, SIZE y MIX globales.
const MIX_FILTER: u8 = 5;
const MIX_DJ_FILT: u8 = 17;
const MIX_SIZE: u8 = 8;
const MIX_MIX: u8 = 7;

/// Subcadenas con las que el aparato se presenta según el firmware y la versión.
///
/// `FM-1` desde JIANT V10; antes `JIANT` / `JIANT FM` / `Felucca`; `SMK` para los teclados de la familia.
pub const KNOWN_NAMES: &[&str] = &["fm-1", "fm1", "jiant", "felucca", "smk"];

fn looks_like_device(name: &str) -> bool {
    let lower = name.to_lowercase();
    KNOWN_NAMES.iter().any(|needle| lower.contains(needle))
}

/// Los nombres de puerto que hay ahora mismo, entrada y salida.
pub fn list() -> Result<(Vec<String>, Vec<String>)> {
    let input = MidiInput::new("fm1-midi")
        .map_err(|e| Error::Transport(format!("CoreMIDI (entrada): {e}")))?;
    let output = MidiOutput::new("fm1-midi")
        .map_err(|e| Error::Transport(format!("CoreMIDI (salida): {e}")))?;
    let ins = input
        .ports()
        .iter()
        .filter_map(|p| input.port_name(p).ok())
        .collect();
    let outs = output
        .ports()
        .iter()
        .filter_map(|p| output.port_name(p).ok())
        .collect();
    Ok((ins, outs))
}

/// Detecta el perfil por identidad OTA + INFO; cae al genérico si ninguno matchea.
///
/// Vive en el adaptador y no en la app: es I/O, y la CLI y la ventana tienen que contestar lo mismo.
pub fn detect_profile(profiles: &[Profile]) -> Result<Profile> {
    let mut port = Port::open()?;
    let identity = port
        .identify(Duration::from_millis(700))
        .unwrap_or_default();
    let version = port
        .editor(&editor::info(), QUERY_WAIT)
        .and_then(|reply| reply.version())
        .unwrap_or_default();
    Ok(Profile::matches(profiles, &identity, &version)
        .cloned()
        .or_else(|| {
            profiles
                .iter()
                .find(|profile| profile.id == "generic")
                .cloned()
        })
        .unwrap_or_else(fm1_core::profile::generic))
}

/// Un aparato abierto: salida conectada y un hilo de entrada que junta lo que llega.
///
/// No hay notificación de hot-plug en midir: si el aparato se desconecta, `send` empieza a fallar y hay
/// que volver a llamar a [`open`]. Por eso el error es explícito y la UI puede ofrecer reconectar.
pub struct Port {
    name: String,
    out: MidiOutputConnection,
    // Se guarda aunque no se use: soltarlo cierra el puerto de entrada.
    _in: MidiInputConnection<()>,
    rx: Receiver<Vec<u8>>,
}

impl std::fmt::Debug for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Port").field("name", &self.name).finish()
    }
}

impl Port {
    /// El nombre con el que se abrió (para mostrarlo en la UI).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Handshake de identidad (lectura pura: no escribe nada en el aparato).
    ///
    /// Devuelve `None` si el firmware no contesta, que es lo normal en firmwares que no implementan el
    /// actualizador por SysEx.
    pub fn identify(&mut self, wait: Duration) -> Option<String> {
        self.send(&fm1_core::message::IDENTITY_REQUEST).ok()?;
        while let Some(packet) = self.recv(wait) {
            if let Some(identity) = fm1_core::message::parse_identity(&packet) {
                return Some(identity);
            }
        }
        None
    }

    /// Manda un pedido del editor protocol y espera la respuesta con el mismo comando.
    ///
    /// Los paquetes que no son del editor (eco de notas, clock) se descartan. `None` si no llegó la
    /// respuesta dentro de `wait`: el llamador decide si eso es "sin aparato" o "firmware viejo".
    pub fn editor(&mut self, request: &[u8], wait: Duration) -> Option<editor::Reply> {
        self.send(request).ok()?;
        let deadline = Instant::now() + wait;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            let packet = self.recv(left)?;
            if editor::answers(request, &packet) {
                return editor::Reply::parse(&packet);
            }
        }
    }

    /// La pista seleccionada y los cuatro valores que muestra el panel, leídos del aparato.
    pub fn mix(&mut self) -> Result<Mix> {
        let selected = self
            .editor(&editor::track_query(), QUERY_WAIT)
            .and_then(|reply| reply.selected_track())
            .ok_or_else(|| Error::Transport("la pista seleccionada no contestó".into()))?;
        Ok(Mix {
            selected,
            filter: self.value(Scope::Track, MIX_FILTER)?,
            dj_filter: self.value(Scope::Global, MIX_DJ_FILT)?,
            size: self.value(Scope::Global, MIX_SIZE)?,
            mix: self.value(Scope::Global, MIX_MIX)?,
        })
    }

    /// Lee un parámetro del perfil. `None` si su protocolo no tiene lectura (CC/NRPN).
    pub fn read_param(&mut self, param: &Param) -> Result<Option<i16>> {
        match param.access {
            ParamAccess::Editor { scope, id } => {
                let reply = self
                    .editor(&editor::get(scope, id), QUERY_WAIT)
                    .ok_or_else(|| Error::Transport(format!("{} no contestó", param.id)))?;
                let (answered, value) = reply.value().ok_or_else(|| {
                    Error::Transport(format!("{}: respuesta inesperada", param.id))
                })?;
                if answered != id {
                    return Err(Error::Transport(format!(
                        "{}: el aparato contestó otro parámetro",
                        param.id
                    )));
                }
                Ok(Some(value))
            }
            ParamAccess::Cc { .. } | ParamAccess::Nrpn { .. } | ParamAccess::None => Ok(None),
        }
    }

    /// Escribe un parámetro del perfil por su acceso. El editor confirma; CC/NRPN no tienen ACK.
    pub fn write_param(&mut self, param: &Param, value: i16) -> Result<()> {
        match param.access {
            ParamAccess::Editor { scope, id } => {
                let reply = self
                    .editor(&editor::set(scope, id, value), QUERY_WAIT)
                    .ok_or_else(|| Error::Transport(format!("{} no confirmó", param.id)))?;
                let (answered, applied) = reply.value().ok_or_else(|| {
                    Error::Transport(format!("{}: respuesta inesperada", param.id))
                })?;
                if answered != id {
                    return Err(Error::Transport(format!(
                        "{}: el aparato confirmó otro parámetro",
                        param.id
                    )));
                }
                if applied != value {
                    return Err(Error::Param(format!(
                        "{}: el aparato aplicó {applied} en vez de {value}",
                        param.id
                    )));
                }
                Ok(())
            }
            ParamAccess::Cc { channel, cc } => {
                let value = value.clamp(0, 127) as u8;
                self.send(&message::cc(channel, cc, value)?)
            }
            ParamAccess::Nrpn { channel, msb, lsb } => {
                let value = scale_param(param.min, param.max, value);
                for msg in [
                    message::cc(channel, 99, msb)?,
                    message::cc(channel, 98, lsb)?,
                    message::cc(channel, 6, ((value >> 7) & 0x7F) as u8)?,
                    message::cc(channel, 38, (value & 0x7F) as u8)?,
                ] {
                    self.send(&msg)?;
                }
                Ok(())
            }
            ParamAccess::None => Err(Error::Param(format!("{} no se puede escribir", param.id))),
        }
    }

    /// Lee los parámetros que el protocolo permita leer; `None` para los que no.
    pub fn watch(&mut self, params: &[Param]) -> Result<Vec<Option<i16>>> {
        let mut values = Vec::with_capacity(params.len());
        for param in params {
            values.push(self.read_param(param)?);
        }
        Ok(values)
    }

    fn value(&mut self, scope: Scope, id: u8) -> Result<i16> {
        self.editor(&editor::get(scope, id), QUERY_WAIT)
            .and_then(|reply| reply.value())
            .filter(|(answered, _)| *answered == id)
            .map(|(_, value)| value)
            .ok_or_else(|| Error::Transport(format!("el parámetro {id} no contestó")))
    }

    /// Abre el primer puerto que se parezca a un FM-1. Ver [`KNOWN_NAMES`].
    pub fn open() -> Result<Self> {
        Self::open_matching(None)
    }

    /// Abre el puerto cuyo nombre contenga `pattern` (sin distinguir mayúsculas), o el primero conocido.
    pub fn open_matching(pattern: Option<&str>) -> Result<Self> {
        let output = MidiOutput::new("fm1-midi-out")
            .map_err(|e| Error::Transport(format!("CoreMIDI (salida): {e}")))?;
        let out_ports = output.ports();
        let wanted = |name: &str| match pattern {
            Some(pat) => name.to_lowercase().contains(&pat.to_lowercase()),
            None => looks_like_device(name),
        };
        let out_port = out_ports
            .iter()
            .find(|p| output.port_name(p).is_ok_and(|n| wanted(&n)))
            .ok_or_else(|| {
                Error::Transport(
                    "no hay puerto MIDI del FM-1 (¿encendido? ¿cable de datos?)".into(),
                )
            })?;
        let name = output
            .port_name(out_port)
            .map_err(|e| Error::Transport(format!("nombre del puerto: {e}")))?;
        let out = output
            .connect(out_port, "fm1-midi")
            .map_err(|e| Error::Transport(format!("no se pudo abrir {name}: {e}")))?;

        let input = MidiInput::new("fm1-midi-in")
            .map_err(|e| Error::Transport(format!("CoreMIDI (entrada): {e}")))?;
        let in_port = input
            .ports()
            .into_iter()
            .find(|p| input.port_name(p).is_ok_and(|n| wanted(&n)))
            .ok_or_else(|| Error::Transport(format!("{name} no expone entrada MIDI")))?;
        let (tx, rx) = channel();
        let conn = input
            .connect(
                &in_port,
                "fm1-midi-in",
                move |_stamp, message, _| {
                    let _ = tx.send(message.to_vec());
                },
                (),
            )
            .map_err(|e| Error::Transport(format!("no se pudo escuchar {name}: {e}")))?;

        Ok(Self {
            name,
            out,
            _in: conn,
            rx,
        })
    }
}

impl MidiOut for Port {
    fn send(&mut self, message: &[u8]) -> Result<()> {
        self.out
            .send(message)
            .map_err(|e| Error::Transport(format!("envío: {e}")))
    }
}

impl MidiIn for Port {
    fn recv(&mut self, timeout: Duration) -> Option<Vec<u8>> {
        self.rx.recv_timeout(timeout).ok()
    }
}

/// Una conexión de mentira con el mismo contrato, para que la app arranque sin aparato.
impl Port {
    /// `true` si el aparato sigue respondiendo (no hay una prueba mejor que mandar un byte inocuo).
    pub fn is_alive(&mut self) -> bool {
        self.send(&[0xB0, 120, 0]).is_ok()
    }
}

/// El reloj maestro: mientras vive, el aparato sigue los pulsos de esta app.
///
/// Con `CLOCK = INT` el firmware ignora `Start`, `Stop` y el clock que llegan por USB. Por eso
/// [`ClockMaster::start`] lee el reloj actual, lo pone en `USB` y recién después arranca; al parar (o si
/// un envío falla, o en `Drop`) deja el `CLOCK` como estaba.
#[derive(Debug)]
pub struct ClockMaster {
    stop: Arc<AtomicBool>,
    tempo: Arc<AtomicU32>,
    error: Arc<Mutex<Option<String>>>,
    handle: Option<JoinHandle<()>>,
}

impl ClockMaster {
    /// Arranca el transporte a `bpm`. Devuelve error si no hay puerto o si el aparato no acepta el reloj.
    pub fn start(bpm: f32) -> Result<Self> {
        let mut port = Port::open()?;
        let original = port
            .editor(&editor::clock_get(), QUERY_WAIT)
            .and_then(|reply| reply.value())
            .and_then(|(_, value)| editor::Clock::from_value(value))
            .unwrap_or(editor::Clock::Int);
        Self::start_with_port(port, original, bpm)
    }

    /// Arranca con un `CLOCK` base explícito. La app guarda el que leyó al arrancar (o el que fijó el
    /// usuario) y restaura ese: así el USB que dejó una sesión anterior no se contagia.
    pub fn start_with(bpm: f32, original: editor::Clock) -> Result<Self> {
        let port = Port::open()?;
        Self::start_with_port(port, original, bpm)
    }

    fn start_with_port(mut port: Port, original: editor::Clock, bpm: f32) -> Result<Self> {
        // La respuesta del SET confirma que el firmware ya cambió el reloj. Sin esperarla, el Start
        // puede llegar antes y descartarse por `CLOCK = INT`.
        port.editor(&editor::clock_set(editor::Clock::Usb), QUERY_WAIT)
            .ok_or_else(|| Error::Transport("el aparato no aceptó CLOCK = USB".into()))?;
        port.send(&message::START)?;
        Ok(Self::spawn(port, Cleanup::Editor { original }, bpm))
    }

    /// Arranca un transporte MIDI genérico: Start + clock, sin tocar ajustes del aparato.
    /// El firmware tiene que estar configurado para seguir el clock por USB.
    pub fn start_midi(bpm: f32) -> Result<Self> {
        let mut port = Port::open()?;
        port.send(&message::START)?;
        Ok(Self::spawn(port, Cleanup::None, bpm))
    }

    fn spawn(port: Port, cleanup: Cleanup, bpm: f32) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let tempo = Arc::new(AtomicU32::new(bpm_to_centi(bpm)));
        let error = Arc::new(Mutex::new(None));

        let thread_stop = Arc::clone(&stop);
        let thread_tempo = Arc::clone(&tempo);
        let thread_error = Arc::clone(&error);
        let handle = thread::spawn(move || {
            clock_loop(port, cleanup, thread_stop, thread_tempo, thread_error);
        });

        Self {
            stop,
            tempo,
            error,
            handle: Some(handle),
        }
    }

    /// Cambia el tempo sin cortar el reloj.
    pub fn set_tempo(&self, bpm: f32) {
        self.tempo.store(bpm_to_centi(bpm), Ordering::Relaxed);
    }

    /// El error del hilo de clock, si se cortó.
    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|slot| slot.clone())
    }

    /// Para el transporte y devuelve el error del hilo, si hubo.
    pub fn stop(mut self) -> Option<String> {
        self.finish()
    }

    fn finish(&mut self) -> Option<String> {
        if let Some(handle) = self.handle.take() {
            self.stop.store(true, Ordering::Release);
            let _ = handle.join();
        }
        self.error()
    }
}

impl Drop for ClockMaster {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

/// Qué dejar como estaba al terminar el transporte.
#[derive(Debug, Clone, Copy)]
enum Cleanup {
    /// JIANT: el BPM del clock y el `CLOCK` original por el editor.
    Editor { original: editor::Clock },
    /// MIDI genérico: sólo el Stop.
    None,
}

fn clock_loop(
    mut port: Port,
    cleanup: Cleanup,
    stop: Arc<AtomicBool>,
    tempo: Arc<AtomicU32>,
    error: Arc<Mutex<Option<String>>>,
) {
    let mut next = Instant::now();
    loop {
        if stop.load(Ordering::Acquire) {
            break;
        }
        let bpm = (tempo.load(Ordering::Relaxed) as f64 / 100.0).max(40.0);
        let interval = Duration::from_secs_f64(60.0 / (bpm * 24.0));

        if let Err(failure) = port.send(&message::CLOCK) {
            if let Ok(mut slot) = error.lock() {
                *slot = Some(format!("se cortó el reloj: {failure}"));
            }
            break;
        }

        next += interval;
        let now = Instant::now();
        if next > now {
            let wait = next - now;
            if wait > Duration::from_millis(2) {
                thread::sleep(wait - Duration::from_millis(1));
            }
            while Instant::now() < next {
                std::hint::spin_loop();
            }
        } else {
            // Atrasado (p. ej. el tempo subió): no acumular deuda, seguir desde ahora.
            next = now;
        }
    }

    let _ = port.send(&message::STOP);
    let Cleanup::Editor { original } = cleanup else {
        return;
    };
    // El firmware estima el BPM del clock y lo guarda: dejarlo en el tempo que usó la app evita que
    // la próxima lectura arranque con unos BPM de error. Cada SET espera su respuesta: dos tramas
    // seguidas y el puerto cerrándose hacen que el aparato se quede con la primera. El reloj se
    // verifica y se reintenta: si el `SET` se pierde, el aparato queda en USB para siempre.
    let bpm = (tempo.load(Ordering::Relaxed) as f64 / 100.0).round() as i16;
    let _ = port.editor(
        &editor::set(fm1_core::Scope::Global, editor::BPM_GLOBAL, bpm),
        QUERY_WAIT,
    );
    for _ in 0..3 {
        let _ = port.editor(&editor::clock_set(original), QUERY_WAIT);
        let applied = port
            .editor(&editor::clock_get(), QUERY_WAIT)
            .and_then(|reply| reply.value())
            .and_then(|(_, value)| editor::Clock::from_value(value));
        if applied == Some(original) {
            break;
        }
        thread::sleep(Duration::from_millis(60));
    }
}

fn bpm_to_centi(bpm: f32) -> u32 {
    (bpm.clamp(40.0, 240.0) * 100.0).round() as u32
}

fn scale_param(min: i16, max: i16, value: i16) -> u16 {
    let span = (max as i32 - min as i32).max(1);
    (((value as i32 - min as i32) * 16383 + span / 2) / span).clamp(0, 16383) as u16
}

#[cfg(test)]
mod tests {
    use super::scale_param;

    #[test]
    fn el_valor_nrpn_se_escala_al_rango_del_parametro() {
        assert_eq!(scale_param(40, 240, 140), 8192);
        assert_eq!(scale_param(0, 127, 0), 0);
        assert_eq!(scale_param(0, 127, 127), 16383);
        assert_eq!(scale_param(-64, 63, 0), 8256);
    }
}

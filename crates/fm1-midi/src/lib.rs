//! `fm1-midi` — el adaptador de `midir` para los puertos que declara `fm1-core`.
//!
//! Es la única capa que conoce CoreMIDI, los nombres de los puertos y el hilo de lectura. El dominio no
//! se entera: recibe un `MidiOut` y un `MidiIn` y sigue.

use fm1_core::transport::{MidiIn, MidiOut};
use fm1_core::{Error, Result};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

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

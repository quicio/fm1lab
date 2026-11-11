//! El enlace con el aparato: descubrir el puerto y mandar intenciones.
//!
//! Es el único lugar de la app que habla con `fm1-midi`. La UI no ve bytes ni puertos: pide intenciones y
//! acá se abren y cierran. Abrir por comando cuesta milisegundos y evita el problema de un puerto que
//! queda tomado cuando el aparato se desconecta (`midir` no tiene hot-plug).

use fm1_core::Result;
use fm1_core::device::Device;
use fm1_core::transport::MidiOut;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Nombres que publican los firmwares abiertos, en minúsculas.
const KNOWN: [&str; 5] = ["fm-1", "fm1", "jiant", "felucca", "smk"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// Hay puerto: se muestra su nombre.
    Ready(String),
    /// No hay aparato: la lista se edita igual, pero nada se envía.
    Absent,
}

/// Sondeo de puertos en un hilo aparte.
///
/// CoreMIDI no avisa cuándo aparece o desaparece un aparato, y cada instalación de firmware lo vuelve a
/// enumerar con otro nombre. Sondear es la única opción, y va fuera del hilo de dibujo porque puede tardar.
#[derive(Debug, Clone)]
pub struct Probe {
    state: Arc<Mutex<Link>>,
}

impl Probe {
    pub fn start() -> Self {
        let state = Arc::new(Mutex::new(Link::Absent));
        let shared = Arc::clone(&state);
        thread::spawn(move || {
            loop {
                let found = fm1_midi::list()
                    .ok()
                    .and_then(|(_, outs)| outs.into_iter().find(|n| looks_like_device(n)))
                    .map(Link::Ready)
                    .unwrap_or(Link::Absent);
                if let Ok(mut slot) = shared.lock()
                    && *slot != found
                {
                    *slot = found;
                }
                thread::sleep(Duration::from_millis(1500));
            }
        });
        Self { state }
    }

    pub fn link(&self) -> Link {
        self.state.lock().map(|g| g.clone()).unwrap_or(Link::Absent)
    }
}

fn looks_like_device(name: &str) -> bool {
    let lower = name.to_lowercase();
    KNOWN.iter().any(|k| lower.contains(k))
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

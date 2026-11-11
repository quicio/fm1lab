//! Los puertos que el dominio necesita, declarados acá y cumplidos afuera.
//!
//! `fm1-core` pide "algo que mande bytes" y "algo que reciba bytes"; `fm1-midi` los cumple con midir.
//! Esa inversión es la que permite testear el dominio sin aparato y cambiar de transporte sin tocar
//! una regla.

use crate::{Error, Result};
use std::time::Duration;

/// Salida de bytes MIDI.
pub trait MidiOut {
    fn send(&mut self, message: &[u8]) -> Result<()>;
}

/// Entrada de mensajes MIDI, con espera acotada.
pub trait MidiIn {
    /// El próximo mensaje completo, o `None` si no llegó nada en `timeout`.
    fn recv(&mut self, timeout: Duration) -> Option<Vec<u8>>;
}

/// Un `MidiOut` que junta todo en memoria: el doble de prueba que usa el dominio en los tests.
#[derive(Debug, Default, Clone)]
pub struct Recorder {
    pub sent: Vec<Vec<u8>>,
}

impl MidiOut for Recorder {
    fn send(&mut self, message: &[u8]) -> Result<()> {
        self.sent.push(message.to_vec());
        Ok(())
    }
}

// Los `Box<dyn MidiOut>` y los préstamos también son salidas: así la app puede elegir el transporte en
// tiempo de ejecución (aparato real, grabador o guion) sin genéricos en la UI.
impl<T: MidiOut + ?Sized> MidiOut for Box<T> {
    fn send(&mut self, message: &[u8]) -> Result<()> {
        (**self).send(message)
    }
}

impl<T: MidiOut + ?Sized> MidiOut for &mut T {
    fn send(&mut self, message: &[u8]) -> Result<()> {
        (**self).send(message)
    }
}

impl Recorder {
    pub fn last(&self) -> Option<&[u8]> {
        self.sent.last().map(Vec::as_slice)
    }
}

/// Un `MidiIn` guionado: devuelve los paquetes cargados y después nada.
#[derive(Debug, Default, Clone)]
pub struct ScriptedIn {
    pub packets: Vec<Vec<u8>>,
    pub pos: usize,
}

impl MidiIn for ScriptedIn {
    fn recv(&mut self, _timeout: Duration) -> Option<Vec<u8>> {
        let packet = self.packets.get(self.pos).cloned();
        if packet.is_some() {
            self.pos += 1;
        }
        packet
    }
}

/// Un aparato que no contesta: para probar el camino de error.
#[derive(Debug, Default, Clone)]
pub struct Silent;

impl MidiOut for Silent {
    fn send(&mut self, _message: &[u8]) -> Result<()> {
        Err(Error::Transport("no hay aparato".into()))
    }
}

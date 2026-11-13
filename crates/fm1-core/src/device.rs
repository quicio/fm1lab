//! Intenciones del dominio: "poné el tempo en 140", "andá a la sección B".
//!
//! [`Device`] traduce eso a mensajes y los entrega al [`MidiOut`] que le inyecten. No sabe si abajo hay
//! USB, TRS o un `Recorder` de prueba.

use crate::message::{self, IDENTITY_REQUEST};
use crate::nrpn::{self, SONG_CHANNEL, Scope};
use crate::setlist::Item;
use crate::transport::{MidiIn, MidiOut};
use crate::{Error, Result, editor};
use std::time::Duration;

/// Una sección de la canción. En el canal 16 es un Program Change de 0 a 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    A,
    B,
    C,
    D,
}

impl Section {
    pub fn program(self) -> u8 {
        match self {
            Section::A => 0,
            Section::B => 1,
            Section::C => 2,
            Section::D => 3,
        }
    }

    pub fn from_letter(c: char) -> Option<Self> {
        match c.to_ascii_uppercase() {
            'A' => Some(Section::A),
            'B' => Some(Section::B),
            'C' => Some(Section::C),
            'D' => Some(Section::D),
            _ => None,
        }
    }
}

/// El aparato, visto como lo ve el dominio: un destino de mensajes.
#[derive(Debug)]
pub struct Device<O: MidiOut> {
    out: O,
    /// Canal de la canción (secciones y proyectos). El firmware lo fija en 16.
    song_channel: u8,
}

impl<O: MidiOut> Device<O> {
    pub fn new(out: O) -> Self {
        Self {
            out,
            song_channel: SONG_CHANNEL,
        }
    }

    pub fn out(&self) -> &O {
        &self.out
    }

    /// Tempo del aparato (global BPM, 40..240) por el editor protocol.
    ///
    /// Va por el editor y no por NRPN porque el firmware 0.9.9 no implementa NRPN: los CC99/98/6/38
    /// sólo cancelan el RPN. El id y el rango son los del mapa; el valor va crudo en offset binary.
    pub fn set_bpm(&mut self, bpm: i16) -> Result<()> {
        self.set_global_editor(editor::BPM_GLOBAL, bpm)
    }

    /// Un global por id, por el editor protocol (el camino que el firmware sí implementa).
    pub fn set_global_editor(&mut self, id: u8, value: i16) -> Result<()> {
        let spec = nrpn::by_id(Scope::Global, id)
            .ok_or_else(|| Error::Param(format!("no existe el global id {id}")))?;
        if value < spec.min || value > spec.max {
            return Err(Error::Param(format!(
                "{} = {value} fuera de {}..{}",
                spec.name, spec.min, spec.max
            )));
        }
        self.emit(&editor::set(Scope::Global, id, value))
    }

    /// Un parámetro de la pista seleccionada, por el editor protocol.
    ///
    /// La pista la elige el aparato: esto no dice "pista 1", dice "la que está seleccionada". La UI
    /// lee cuál es con `TRACK` y lo muestra.
    pub fn set_selected_track(&mut self, id: u8, value: i16) -> Result<()> {
        let spec = nrpn::by_id(Scope::Track, id)
            .ok_or_else(|| Error::Param(format!("no existe el parámetro de pista {id}")))?;
        if value < spec.min || value > spec.max {
            return Err(Error::Param(format!(
                "{} = {value} fuera de {}..{}",
                spec.name, spec.min, spec.max
            )));
        }
        self.emit(&editor::set(Scope::Track, id, value))
    }

    /// Pone la fuente de reloj del aparato (por el editor protocol, no por NRPN).
    pub fn set_clock(&mut self, clock: editor::Clock) -> Result<()> {
        self.emit(&editor::clock_set(clock))
    }

    /// Arranca el transporte. Sólo lo atiende el firmware si el reloj es USB o TRS.
    pub fn start(&mut self) -> Result<()> {
        self.emit(&message::START)
    }

    /// Para el transporte.
    pub fn stop(&mut self) -> Result<()> {
        self.emit(&message::STOP)
    }

    /// Un pulso de clock (24 por negra).
    pub fn pulse(&mut self) -> Result<()> {
        self.emit(&message::CLOCK)
    }

    /// Un global por nombre (`BPM`, `SWING`, `DJ FILT`, `SIZE`, …), con el rango validado contra el mapa.
    pub fn set_global(&mut self, name: &str, value: i16) -> Result<()> {
        let spec = nrpn::global_by_name(name)
            .ok_or_else(|| Error::Param(format!("no existe el global {name:?}")))?;
        let scaled = nrpn::to_14bit(&spec, value)?;
        self.emit_nrpn(self.song_channel, Scope::Global, spec.id, scaled)
    }

    /// Un global por id, cuando el nombre no alcanza.
    pub fn set_global_id(&mut self, id: u8, value: i16) -> Result<()> {
        let spec = nrpn::by_id(Scope::Global, id)
            .ok_or_else(|| Error::Param(format!("no existe el global id {id}")))?;
        let scaled = nrpn::to_14bit(&spec, value)?;
        self.emit_nrpn(self.song_channel, Scope::Global, spec.id, scaled)
    }

    /// Un parámetro de pista, en el canal que la pista toca (T1..T6 = canales 1..6).
    ///
    /// Ojo: si el ROUTE del aparato no hace que la pista toque ese canal, esto no hace nada y no avisa.
    pub fn set_track(&mut self, track: u8, name: &str, value: i16) -> Result<()> {
        if !(1..=6).contains(&track) {
            return Err(Error::Param(format!("pista {track} fuera de 1..=6")));
        }
        let spec = nrpn::track_by_name(name)
            .ok_or_else(|| Error::Param(format!("no existe el parámetro de pista {name:?}")))?;
        let scaled = nrpn::to_14bit(&spec, value)?;
        self.emit_nrpn(track, Scope::Track, spec.id, scaled)
    }

    /// Salta a una sección de la canción (parado: ya; sonando: al compás siguiente).
    pub fn goto_section(&mut self, section: Section) -> Result<()> {
        self.emit(&message::program_change(
            self.song_channel,
            section.program(),
        )?)
    }

    /// Carga un proyecto (1..=6) con su sección A. Sólo tiene efecto con el transporte parado.
    pub fn load_project(&mut self, project: u8) -> Result<()> {
        if !(1..=6).contains(&project) {
            return Err(Error::Param(format!("proyecto {project} fuera de 1..=6")));
        }
        self.emit(&message::program_change(self.song_channel, 15 + project)?)
    }

    /// Carga el sonido `program` en una pista (lista PRESETS, 0..=127).
    pub fn load_sound(&mut self, track: u8, program: u8) -> Result<()> {
        if !(1..=6).contains(&track) {
            return Err(Error::Param(format!("pista {track} fuera de 1..=6")));
        }
        self.emit(&message::program_change(track, program)?)
    }

    /// Toca un elemento de setlist.
    pub fn play(&mut self, item: &Item) -> Result<()> {
        match *item {
            Item::Section(letter) => {
                let section = Section::from_letter(letter)
                    .ok_or_else(|| Error::Param(format!("sección {letter:?} inválida")))?;
                self.goto_section(section)
            }
            Item::Project { project, section } => {
                let section = Section::from_letter(section)
                    .ok_or_else(|| Error::Param(format!("sección {section:?} inválida")))?;
                self.load_project(project)?;
                if section != Section::A {
                    self.goto_section(section)?;
                }
                Ok(())
            }
            Item::Sound { track, program } => self.load_sound(track, program),
        }
    }

    /// Pánico: All Sound Off y All Notes Off en todas las pistas y en la canción.
    pub fn panic(&mut self) -> Result<()> {
        for channel in 1..=16 {
            self.emit(&message::cc(channel, 120, 0)?)?;
            self.emit(&message::cc(channel, 123, 0)?)?;
        }
        Ok(())
    }

    /// Handshake de identidad (lectura). `None` si el firmware no contesta.
    pub fn identify<I: MidiIn>(&mut self, input: &mut I, wait: Duration) -> Option<String> {
        if self.emit(&IDENTITY_REQUEST).is_err() {
            return None;
        }
        while let Some(packet) = input.recv(wait) {
            if let Some(identity) = message::parse_identity(&packet) {
                return Some(identity);
            }
        }
        None
    }

    fn emit_nrpn(&mut self, channel: u8, scope: Scope, id: u8, value: u16) -> Result<()> {
        for msg in message::nrpn(channel, scope, id, value)? {
            self.emit(&msg)?;
        }
        Ok(())
    }

    fn emit(&mut self, bytes: &[u8]) -> Result<()> {
        self.out.send(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::Recorder;

    fn device() -> Device<Recorder> {
        Device::new(Recorder::default())
    }

    #[test]
    fn el_tempo_va_por_el_editor_en_offset_binary() {
        let mut d = device();
        d.set_bpm(140).unwrap();
        assert_eq!(
            d.out().sent,
            vec![vec![0xF0, 0x7D, 0x46, 0x4C, 3, 1, 0, 0x0C, 0x41, 0xF7]]
        );
        assert!(d.set_bpm(300).is_err(), "el rango del mapa se valida");
    }

    #[test]
    fn los_parametros_del_panel_van_por_el_editor_con_su_id_y_rango() {
        let mut d = device();
        d.set_global_editor(8, 91).unwrap(); // SIZE
        assert_eq!(
            d.out().last(),
            Some(&[0xF0, 0x7D, 0x46, 0x4C, 3, 1, 8, 0x5B, 0x40, 0xF7][..])
        );
        d.set_selected_track(5, 10).unwrap(); // FILTER de la pista seleccionada
        assert_eq!(
            d.out().last(),
            Some(&[0xF0, 0x7D, 0x46, 0x4C, 3, 0, 5, 0x0A, 0x40, 0xF7][..])
        );
        assert!(d.set_selected_track(5, 200).is_err());
        assert!(d.set_global_editor(200, 0).is_err());
    }

    #[test]
    fn una_seccion_es_un_program_change_en_el_canal_16() {
        let mut d = device();
        d.goto_section(Section::B).unwrap();
        assert_eq!(d.out().last(), Some(&[0xCF, 0x01][..]));
    }

    #[test]
    fn un_proyecto_es_program_change_16_mas_el_numero() {
        let mut d = device();
        d.load_project(3).unwrap();
        assert_eq!(d.out().last(), Some(&[0xCF, 0x12][..]));
        assert!(d.load_project(7).is_err());
    }

    #[test]
    fn un_parametro_de_pista_va_en_el_canal_de_esa_pista() {
        let mut d = device();
        d.set_track(2, "FILTER", 0).unwrap();
        assert_eq!(d.out().sent[0][0], 0xB1, "pista 2 = canal 2");
    }

    #[test]
    fn el_panico_cubre_los_16_canales() {
        let mut d = device();
        d.panic().unwrap();
        assert_eq!(d.out().sent.len(), 32);
        assert_eq!(d.out().sent[0], vec![0xB0, 120, 0]);
    }

    #[test]
    fn un_global_inexistente_falla_antes_de_emitir() {
        let mut d = device();
        assert!(d.set_global("NO EXISTE", 1).is_err());
        assert!(d.out().sent.is_empty(), "no debe mandar nada a medias");
    }

    #[test]
    fn el_reloj_y_el_transporte_son_los_bytes_del_contrato() {
        let mut d = device();
        d.set_clock(editor::Clock::Usb).unwrap();
        assert_eq!(
            d.out().last(),
            Some(&[0xF0, 0x7D, 0x46, 0x4C, 3, 1, 2, 0x01, 0x40, 0xF7][..])
        );
        d.start().unwrap();
        d.pulse().unwrap();
        d.stop().unwrap();
        assert_eq!(
            d.out().sent[1..],
            [vec![0xFA], vec![0xF8], vec![0xFC]],
            "start, pulso de clock, stop"
        );
    }
}

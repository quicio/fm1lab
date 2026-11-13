//! El editor protocol del firmware JIANT: tramas SysEx y valores, sin I/O.
//!
//! Es un contrato distinto del NRPN: las tramas van `F0 7D 46 4C cmd args F7` y los valores viajan en
//! **offset binary** (`valor + 8192`), no escalados sobre el rango. Mezclar los dos protocolos es el
//! error más fácil de cometer: un `SET` de `CLOCK` con el valor NRPN escribe `INT` cuando se quiere
//! `USB`. Acá sólo se arman y se leen bytes; abrir el puerto es del adaptador.
//!
//! Fuente: `web/EDITOR_PROTOCOL.md` del firmware y capturas hechas contra un par12 (`FM-1_909`).

use crate::Result;
use crate::nrpn::Scope;

/// Cabecera de toda trama del editor.
pub const HEADER: [u8; 4] = [0xF0, 0x7D, 0x46, 0x4C];
/// Fin de SysEx.
pub const END: u8 = 0xF7;

/// Comandos que usa este repo (los números son parte del contrato congelado).
pub mod cmd {
    pub const INFO: u8 = 1;
    pub const GET: u8 = 2;
    pub const SET: u8 = 3;
    pub const PROJECT: u8 = 9;
    pub const TRACK: u8 = 27;
    pub const SONG: u8 = 33;
}

/// El global `CLOCK` (fuente de reloj), fuera del mapa NRPN por ser del transporte.
pub const CLOCK_GLOBAL: u8 = 2;
/// El global `BPM`, primero de la tabla (el mismo id del mapa NRPN).
pub const BPM_GLOBAL: u8 = 0;

/// Fuente del reloj del aparato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clock {
    Int,
    Usb,
    Trs,
}

impl Clock {
    pub fn value(self) -> i16 {
        match self {
            Clock::Int => 0,
            Clock::Usb => 1,
            Clock::Trs => 2,
        }
    }

    pub fn from_value(value: i16) -> Option<Self> {
        match value {
            0 => Some(Clock::Int),
            1 => Some(Clock::Usb),
            2 => Some(Clock::Trs),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Clock::Int => "INT",
            Clock::Usb => "USB",
            Clock::Trs => "TRS",
        }
    }
}

/// Una fila de la cadena de song: sección y compases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainRow {
    pub section: char,
    pub bars: u8,
}

/// La cadena de song que devuelve el aparato. `rows` vacío = no hay cadena configurada.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SongChain {
    pub running: bool,
    pub rows: Vec<ChainRow>,
}

/// Una respuesta del editor, ya validada: el comando y sus argumentos (sin cabecera ni fin).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub cmd: u8,
    pub args: Vec<u8>,
}

impl Reply {
    /// `None` si el paquete no es una respuesta del editor o está cortado.
    pub fn parse(packet: &[u8]) -> Option<Self> {
        if packet.len() < HEADER.len() + 2 || packet[packet.len() - 1] != END {
            return None;
        }
        if packet[..HEADER.len()] != HEADER {
            return None;
        }
        Some(Self {
            cmd: packet[HEADER.len()],
            args: packet[HEADER.len() + 1..packet.len() - 1].to_vec(),
        })
    }

    /// El texto de versión de `INFO` (`JIANT v0.9.9-par12`), hasta el primer cero.
    pub fn version(&self) -> Option<String> {
        if self.cmd != cmd::INFO || self.args.is_empty() {
            return None;
        }
        let text: String = self
            .args
            .iter()
            .take_while(|b| **b != 0)
            .map(|b| *b as char)
            .collect();
        (!text.is_empty()).then_some(text)
    }

    /// `(op, slot, usado)` de una respuesta a `PROJECT` (op 0 carga, 1 guarda, 2 consulta).
    pub fn project(&self) -> Option<(u8, u8, bool)> {
        if self.cmd != cmd::PROJECT || self.args.len() < 3 {
            return None;
        }
        Some((self.args[0], self.args[1], self.args[2] != 0))
    }

    /// `(slot, usado)` de una respuesta a `PROJECT op 2`.
    pub fn project_used(&self) -> Option<(u8, bool)> {
        self.project()
            .filter(|(op, _, _)| *op == 2)
            .map(|(_, slot, used)| (slot, used))
    }

    /// `(id, valor)` de una respuesta a `GET` o al eco de `SET`.
    pub fn value(&self) -> Option<(u8, i16)> {
        if !matches!(self.cmd, cmd::GET | cmd::SET) || self.args.len() < 4 {
            return None;
        }
        Some((self.args[1], wire_to_value(self.args[2], self.args[3])))
    }

    /// La pista seleccionada en el aparato (0..5) de una respuesta a `TRACK`.
    pub fn selected_track(&self) -> Option<u8> {
        if self.cmd != cmd::TRACK || self.args.len() < 2 {
            return None;
        }
        let (selected, count) = (self.args[0], self.args[1]);
        (count > 0 && selected < count).then_some(selected)
    }

    /// La cadena de song de una respuesta a `SONG op 0`.
    pub fn song(&self) -> Option<SongChain> {
        if self.cmd != cmd::SONG || self.args.len() < 6 || self.args[0] != 0 {
            return None;
        }
        let count = self.args[2] as usize;
        if self.args.len() < 6 + count * 2 {
            return None;
        }
        let mut rows = Vec::with_capacity(count);
        for i in 0..count {
            let slot = self.args[6 + i * 2];
            let bars = self.args[7 + i * 2];
            if slot > 3 || bars == 0 {
                return None;
            }
            rows.push(ChainRow {
                section: (b'A' + slot) as char,
                bars,
            });
        }
        Some(SongChain {
            running: self.args[3] != 0,
            rows,
        })
    }
}

/// Trama de `INFO` (versión del firmware).
pub fn info() -> Vec<u8> {
    frame(cmd::INFO, &[])
}

/// Trama de `GET` para un parámetro o global.
pub fn get(scope: Scope, id: u8) -> Vec<u8> {
    frame(cmd::GET, &[scope.msb(), id])
}

/// Trama de `SET`. El valor va en offset binary (no escalado como el NRPN).
pub fn set(scope: Scope, id: u8, value: i16) -> Vec<u8> {
    let wire = value_to_wire(value);
    frame(
        cmd::SET,
        &[scope.msb(), id, (wire & 0x7F) as u8, (wire >> 7) as u8],
    )
}

/// Trama para preguntar si la sección `slot` (0 = A) tiene música.
pub fn project_query(slot: u8) -> Vec<u8> {
    frame(cmd::PROJECT, &[2, slot & 3])
}

/// Trama para cargar la sección `slot` (0 = A). La respuesta confirma el slot y si tenía música.
pub fn project_load(slot: u8) -> Vec<u8> {
    frame(cmd::PROJECT, &[0, slot & 3])
}

/// Trama para leer la cadena de song.
pub fn song_query() -> Vec<u8> {
    frame(cmd::SONG, &[0])
}

/// Trama para leer la pista seleccionada.
pub fn track_query() -> Vec<u8> {
    frame(cmd::TRACK, &[])
}

/// `true` si el paquete es una respuesta al `request` (mismo `cmd`).
pub fn answers(request: &[u8], packet: &[u8]) -> bool {
    Reply::parse(packet)
        .is_some_and(|reply| request.len() > HEADER.len() && reply.cmd == request[HEADER.len()])
}

/// Pide el reloj actual. El consumidor usa [`Reply::global_value`] y [`Clock::from_value`].
pub fn clock_get() -> Vec<u8> {
    get(Scope::Global, CLOCK_GLOBAL)
}

/// Pone una fuente de reloj.
pub fn clock_set(clock: Clock) -> Vec<u8> {
    set(Scope::Global, CLOCK_GLOBAL, clock.value())
}

fn frame(command: u8, args: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HEADER.len() + 1 + args.len() + 1);
    bytes.extend_from_slice(&HEADER);
    bytes.push(command);
    bytes.extend_from_slice(args);
    bytes.push(END);
    bytes
}

/// Un int16 del dominio a los dos bytes del contrato (valor + 8192, 14 bits).
fn value_to_wire(value: i16) -> u16 {
    (value.clamp(-8192, 8191) + 8192) as u16
}

fn wire_to_value(lo: u8, hi: u8) -> i16 {
    ((lo & 0x7F) as i16 | ((hi & 0x7F) as i16) << 7) - 8192
}

/// Valida y codifica una sección (`A`..`D`) como slot.
pub fn section_slot(section: char) -> Result<u8> {
    match section.to_ascii_uppercase() {
        'A' => Ok(0),
        'B' => Ok(1),
        'C' => Ok(2),
        'D' => Ok(3),
        other => Err(crate::Error::Param(format!("sección {other:?} inválida"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_tramas_son_las_del_contrato() {
        assert_eq!(info(), [0xF0, 0x7D, 0x46, 0x4C, 1, 0xF7]);
        assert_eq!(project_query(0), [0xF0, 0x7D, 0x46, 0x4C, 0x09, 2, 0, 0xF7]);
        assert_eq!(song_query(), [0xF0, 0x7D, 0x46, 0x4C, 0x21, 0, 0xF7]);
    }

    #[test]
    fn el_valor_del_editor_va_en_offset_binary() {
        // CLOCK = USB: 1 + 8192 = 8193 = 0x2001 → bytes 0x01, 0x40.
        assert_eq!(
            clock_set(Clock::Usb),
            [0xF0, 0x7D, 0x46, 0x4C, 3, 1, 2, 0x01, 0x40, 0xF7]
        );
        assert_eq!(Clock::Usb.value(), 1);
        assert_eq!(Clock::from_value(2), Some(Clock::Trs));
    }

    #[test]
    fn se_leen_las_capturas_reales_de_la_unidad() {
        let project = [0x7D, 0x46, 0x4C, 9, 2, 0, 0];
        let mut packet = vec![0xF0];
        packet.extend_from_slice(&project);
        packet.push(0xF7);
        let reply = Reply::parse(&packet).unwrap();
        assert_eq!(reply.project_used(), Some((0, false)));

        let song = [
            0x7D, 0x46, 0x4C, 33, 0, 0, 4, 0, 0, 0, 0, 8, 1, 8, 2, 8, 3, 8,
        ];
        let mut packet = vec![0xF0];
        packet.extend_from_slice(&song);
        packet.push(0xF7);
        let chain = Reply::parse(&packet).unwrap().song().unwrap();
        assert_eq!(
            chain.rows,
            vec![
                ChainRow {
                    section: 'A',
                    bars: 8
                },
                ChainRow {
                    section: 'B',
                    bars: 8
                },
                ChainRow {
                    section: 'C',
                    bars: 8
                },
                ChainRow {
                    section: 'D',
                    bars: 8
                },
            ]
        );
        assert!(!chain.running);

        // GET del reloj: 0x00 0x40 = 8192 → valor 0 (INT).
        let mut packet = vec![0xF0, 0x7D, 0x46, 0x4C, 2, 1, 2, 0x00, 0x40, 0xF7];
        assert_eq!(Reply::parse(&packet).unwrap().value(), Some((2, 0)));
        assert_eq!(Clock::from_value(0), Some(Clock::Int));

        packet = vec![0xF0, 0x7D, 0x46, 0x4C, 2, 1, 2, 0x01, 0x40, 0xF7];
        assert_eq!(Reply::parse(&packet).unwrap().value(), Some((2, 1)));

        // TRACK: pista seleccionada 0, seis pistas.
        let mut packet = vec![0xF0, 0x7D, 0x46, 0x4C, 27, 0, 6, 0xF7];
        assert_eq!(Reply::parse(&packet).unwrap().selected_track(), Some(0));
        packet = vec![0xF0, 0x7D, 0x46, 0x4C, 27, 9, 6, 0xF7];
        assert_eq!(Reply::parse(&packet).unwrap().selected_track(), None);
    }

    #[test]
    fn una_respuesta_cualquiera_o_cortada_no_se_confunde() {
        assert!(Reply::parse(&[0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7]).is_none());
        assert!(Reply::parse(&[0xF0, 0x7D, 0x46, 0x4C, 9]).is_none());
    }

    #[test]
    fn el_valor_vuelve_igual_despues_de_ida_y_vuelta() {
        for value in [-8192, -1, 0, 40, 120, 240, 8191] {
            let wire = value_to_wire(value);
            assert_eq!(wire_to_value((wire & 0x7F) as u8, (wire >> 7) as u8), value);
        }
        assert_eq!(value_to_wire(-9000), 0);
        assert_eq!(value_to_wire(9000), 0x3FFF);
    }
}

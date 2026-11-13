//! Codificación de mensajes MIDI. Bytes, nada más: sin puertos ni estado.
//!
//! Los números salen del manual del firmware (§14) y del mapa NRPN (`docs/NRPN.md`).

use crate::nrpn::Scope;
use crate::{Error, Result};

/// Handshake de identidad del actualizador (lectura pura: no escribe nada en el aparato).
pub const IDENTITY_REQUEST: [u8; 10] = [0xF0, 0x00, 0x32, 0x45, 0x00, 0x00, 0x00, 0x40, 0x7F, 0xF7];

/// Realtime: arranca el transporte desde el paso cero.
pub const START: [u8; 1] = [0xFA];
/// Realtime: para el transporte.
pub const STOP: [u8; 1] = [0xFC];
/// Realtime: un pulso de clock (24 por negra).
pub const CLOCK: [u8; 1] = [0xF8];

/// Cambio de control: CC `ctrl` = `value` en `channel` (1..=16).
pub fn cc(channel: u8, ctrl: u8, value: u8) -> Result<[u8; 3]> {
    let status = status(0xB0, channel)?;
    Ok([status, ctrl & 0x7F, value & 0x7F])
}

/// Program Change. En un canal de pista carga un sonido; en el canal 16, sección o proyecto.
pub fn program_change(channel: u8, program: u8) -> Result<[u8; 2]> {
    Ok([status(0xC0, channel)?, program & 0x7F])
}

/// Nota con su velocidad (útil para el rescate y para probar que el aparato suena).
pub fn note_on(channel: u8, note: u8, velocity: u8) -> Result<[u8; 3]> {
    Ok([status(0x90, channel)?, note & 0x7F, velocity & 0x7F])
}

/// Note off.
pub fn note_off(channel: u8, note: u8) -> Result<[u8; 3]> {
    Ok([status(0x80, channel)?, note & 0x7F, 0])
}

/// Un NRPN de 14 bits, en los cuatro mensajes que el aparato espera, en orden.
///
/// `value` va de 0 a 16383; para un parámetro concreto usar [`crate::nrpn::to_14bit`].
pub fn nrpn(channel: u8, scope: Scope, param_id: u8, value: u16) -> Result<[[u8; 3]; 4]> {
    let value = value.min(16383);
    Ok([
        cc(channel, 99, scope.msb())?,     // NRPN MSB: espacio de parámetros
        cc(channel, 98, param_id & 0x7F)?, // NRPN LSB: id
        cc(channel, 6, ((value >> 7) & 0x7F) as u8)?, // data entry MSB
        cc(channel, 38, (value & 0x7F) as u8)?, // data entry LSB
    ])
}

/// Cierra el NRPN seleccionado (CC99 127), para que un CC6 suelto no lo siga moviendo.
pub fn nrpn_off(channel: u8) -> Result<[u8; 3]> {
    cc(channel, 99, 127)
}

fn status(base: u8, channel: u8) -> Result<u8> {
    if !(1..=16).contains(&channel) {
        return Err(Error::Message(format!("canal {channel} fuera de 1..=16")));
    }
    Ok(base | (channel - 1))
}

/// Decodifica la respuesta al handshake: `00 59 11` y el texto en 6..33, con el payload a 7 bits.
pub fn parse_identity(packet: &[u8]) -> Option<String> {
    if packet.len() < 3 || packet[0] != 0xF0 || packet[packet.len() - 1] != 0xF7 {
        return None;
    }
    let mut unpacked = Vec::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for byte in &packet[1..packet.len() - 1] {
        acc |= ((byte & 0x7F) as u32) << bits;
        bits += 7;
        while bits >= 8 {
            unpacked.push((acc & 0xFF) as u8);
            acc >>= 8;
            bits -= 8;
        }
    }
    if unpacked.len() < 34 || unpacked[..3] != [0x00, 0x59, 0x11] {
        return None;
    }
    let text: String = unpacked[6..33]
        .iter()
        .take_while(|b| **b != 0)
        .map(|b| *b as char)
        .collect();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_bytes_son_los_que_verifique_contra_el_mapa() {
        // NRPN global crudo con el valor 140 (el escalado del rango lo hace `nrpn::to_14bit`).
        let msgs = nrpn(16, Scope::Global, 0, 140).unwrap();
        assert_eq!(msgs[0], [0xBF, 0x63, 0x01]);
        assert_eq!(msgs[1], [0xBF, 0x62, 0x00]);
        assert_eq!(msgs[2], [0xBF, 0x06, 0x01]);
        assert_eq!(msgs[3], [0xBF, 0x26, 0x0C]);
    }

    #[test]
    fn program_change_en_el_canal_de_la_cancion() {
        assert_eq!(program_change(16, 0).unwrap(), [0xCF, 0x00]); // sección A
        assert_eq!(program_change(16, 18).unwrap(), [0xCF, 0x12]); // proyecto 3
    }

    #[test]
    fn un_canal_invalido_no_produce_bytes() {
        assert!(cc(0, 74, 100).is_err());
        assert!(cc(17, 74, 100).is_err());
    }

    #[test]
    fn el_handshake_decodifica_una_identidad_real() {
        // Capturado de un FM-1 con stock V15.
        let reply = [
            0xF0, 0x00, 0x32, 0x45, 0x58, 0x01, 0x00, 0x00, 0x23, 0x4D, 0x5A, 0x44, 0x79, 0x05,
            0x26, 0x4C, 0x1A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x06, 0xF7,
        ];
        assert_eq!(parse_identity(&reply).as_deref(), Some("FM-1_015"));
    }

    #[test]
    fn una_respuesta_cualquiera_no_se_confunde_con_una_identidad() {
        assert!(parse_identity(&[0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7]).is_none());
    }
}

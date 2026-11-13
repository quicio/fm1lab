//! El mapa NRPN del firmware JIANT, generado desde `docs/NRPN.md` (ver `scripts/gen_nrpn.py`).
//!
//! Dos espacios de parámetros, elegidos por CC99:
//! - [`Scope::Track`] (`CC99 0`): parámetros de una pista, en el canal que esa pista toca.
//! - [`Scope::Global`] (`CC99 1`): globales (tempo, efectos, buses), en el canal 16.

use crate::{Error, Result};

/// Un parámetro y su rango útil, tal como lo publica el firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamSpec {
    pub id: u8,
    pub name: &'static str,
    pub min: i16,
    pub max: i16,
    pub default: i16,
}

/// Un parámetro de un motor concreto (E1..E8): el mismo id se llama distinto en cada motor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineParamSpec {
    pub engine: &'static str,
    pub id: u8,
    pub name: &'static str,
    pub min: i16,
    pub max: i16,
    pub default: i16,
}

/// Qué espacio de parámetros se direcciona: el CC99 que precede al CC98.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Track,
    Global,
}

impl Scope {
    pub fn msb(self) -> u8 {
        match self {
            Scope::Track => 0,
            Scope::Global => 1,
        }
    }
}

/// El canal de la canción: sus Program Change cambian de sección o de proyecto, nunca de sonido.
pub const SONG_CHANNEL: u8 = 16;

/// Primer canal de pista (T1..T6 viven en 1..=6).
pub const FIRST_TRACK_CHANNEL: u8 = 1;

/// Busca un parámetro global por nombre, sin distinguir mayúsculas.
pub fn global_by_name(name: &str) -> Option<ParamSpec> {
    find(crate::nrpn_table::GLOBAL, name)
}

/// Busca un parámetro de pista por nombre, sin distinguir mayúsculas.
pub fn track_by_name(name: &str) -> Option<ParamSpec> {
    find(crate::nrpn_table::TRACK, name)
}

/// Busca un parámetro por id en el espacio pedido.
pub fn by_id(scope: Scope, id: u8) -> Option<ParamSpec> {
    let table = match scope {
        Scope::Track => crate::nrpn_table::TRACK,
        Scope::Global => crate::nrpn_table::GLOBAL,
    };
    table.iter().copied().find(|p| p.id == id)
}

/// Los E1..E8 de un motor.
pub fn engine_params(engine: &str) -> Vec<EngineParamSpec> {
    crate::nrpn_table::ENGINE
        .iter()
        .copied()
        .filter(|p| p.engine.eq_ignore_ascii_case(engine))
        .collect()
}

fn find(table: &'static [ParamSpec], name: &str) -> Option<ParamSpec> {
    table
        .iter()
        .copied()
        .find(|p| p.name.eq_ignore_ascii_case(name))
}

/// Lleva un valor del rango del parámetro a los 14 bits del NRPN, sin salirse de la escala.
///
/// `v = min + (x * (max - min) + 8191) / 16383`, que es la fórmula que documenta el firmware.
pub fn to_14bit(spec: &ParamSpec, value: i16) -> Result<u16> {
    if value < spec.min || value > spec.max {
        return Err(Error::Param(format!(
            "{} = {value} fuera de {}..{}",
            spec.name, spec.min, spec.max
        )));
    }
    if spec.max == spec.min {
        return Ok(0);
    }
    let span = (spec.max - spec.min) as i32;
    let rel = (value - spec.min) as i32;
    Ok(((rel * 16383 + span / 2) / span).clamp(0, 16383) as u16)
}

/// El valor que corresponde a un NRPN de 14 bits, para pintar el estado real del aparato.
pub fn from_14bit(spec: &ParamSpec, x: u16) -> i16 {
    let span = (spec.max - spec.min) as i32;
    let x = (x.min(16383) as i32).min(16383);
    spec.min + ((x * span + 8191) / 16383) as i16
}

/// Cuántos parámetros hay en el mapa, por espacio. Sirve de humo en los tests.
pub fn count(scope: Scope) -> usize {
    table(scope).len()
}

/// La tabla completa de un espacio, para listarla o volcarla.
pub fn table(scope: Scope) -> &'static [ParamSpec] {
    match scope {
        Scope::Track => crate::nrpn_table::TRACK,
        Scope::Global => crate::nrpn_table::GLOBAL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_tabla_tiene_los_parametros_que_documenta_el_firmware() {
        assert!(count(Scope::Track) >= 80, "track: {}", count(Scope::Track));
        assert!(
            count(Scope::Global) >= 60,
            "global: {}",
            count(Scope::Global)
        );
    }

    #[test]
    fn conoce_los_globales_de_performance() {
        let bpm = global_by_name("BPM").expect("BPM");
        assert_eq!((bpm.min, bpm.max, bpm.default), (40, 240, 120));
        assert!(global_by_name("DJ FILT").is_some());
        assert!(global_by_name("SWING").is_some());
    }

    #[test]
    fn los_14_bits_van_y_vuelven() {
        let bpm = global_by_name("BPM").unwrap();
        for v in [40, 120, 140, 240] {
            let x = to_14bit(&bpm, v).unwrap();
            assert_eq!(from_14bit(&bpm, x), v, "bpm {v} pasó por {x}");
        }
    }

    #[test]
    fn un_valor_fuera_de_rango_es_error_de_dominio() {
        let bpm = global_by_name("BPM").unwrap();
        assert!(matches!(to_14bit(&bpm, 300), Err(Error::Param(_))));
    }

    #[test]
    fn los_motores_listan_sus_ocho_valores() {
        let fm6 = engine_params("FM6");
        assert_eq!(fm6.len(), 8, "{fm6:?}");
        assert_eq!(fm6[0].id, 115);
        assert_eq!(fm6[0].name, "ALG");
    }
}

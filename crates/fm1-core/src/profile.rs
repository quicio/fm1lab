//! Perfiles de firmware: qué expone cada uno y cómo se llega.
//!
//! El lab no habla un protocolo: habla **el perfil**. Un perfil declara identidad, parámetros con su
//! acceso (`cc`, `nrpn`, `editor`), transporte y escenas. `jiant` y `generic` son built-in; los del
//! usuario son JSON (ver `fm1-store`). Este módulo es data pura: no abre puertos ni archivos.

use crate::nrpn::{self, Scope};
use serde::{Deserialize, Serialize};

/// Versión del formato de perfil.
pub const VERSION: u32 = 1;

/// Cómo se lee/escribe un parámetro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParamAccess {
    /// Control Change: canal 1..16, número y valor 0..127.
    Cc { channel: u8, cc: u8 },
    /// NRPN de 14 bits (CC99/98 + CC6/38).
    Nrpn { channel: u8, msb: u8, lsb: u8 },
    /// El editor SysEx de JIANT (offset binary, GET/SET).
    Editor { scope: Scope, id: u8 },
    /// Sólo se muestra; no se puede escribir desde el lab.
    None,
}

impl ParamAccess {
    pub fn is_readable(self) -> bool {
        matches!(self, ParamAccess::Editor { .. })
    }
}

/// Un parámetro del catálogo del perfil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Param {
    /// Id estable dentro del perfil (`t1.level`, `g.size`, `t5.filter`).
    pub id: String,
    /// Nombre corto que se muestra en la franja (`LEVEL`, `SIZE`).
    pub label: String,
    pub scope: Scope,
    pub min: i16,
    pub max: i16,
    pub default: i16,
    pub access: ParamAccess,
}

impl Param {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        scope: Scope,
        min: i16,
        max: i16,
        default: i16,
        access: ParamAccess,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            scope,
            min,
            max,
            default,
            access,
        }
    }

    pub fn contains(&self, value: i16) -> bool {
        (self.min..=self.max).contains(&value)
    }
}

/// Cómo arranca el transporte con este firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransportSpec {
    /// La app manda Start/clock; si el firmware necesita un ajuste, lo declara acá.
    ClockMaster { clock: Option<ParamAccess> },
    /// Start/Stop/clock genéricos, sin tocar ajustes del aparato.
    Midi,
    /// El firmware no sigue un transporte externo.
    None,
}

/// Qué puede disparar el setlist/escenas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SceneSpec {
    /// PC en un canal: secciones y proyectos (JIANT).
    ProgramChange {
        channel: u8,
        sections: Vec<u8>,
        projects: Vec<u8>,
    },
    /// PC que elige patrón (firmwares de groovebox).
    Patterns { channel: u8, first: u8, count: u8 },
    /// Sin escenas declaradas; la UI no muestra setlist.
    None,
}

/// Un firmware, descrito.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub version: u32,
    pub id: String,
    pub name: String,
    /// Subcadenas que reconocen el firmware en la identidad o en la versión del editor.
    #[serde(default)]
    pub matches: Vec<String>,
    pub params: Vec<Param>,
    /// Ids de los cuatro parámetros que arrancan en las franjas.
    #[serde(default)]
    pub default_strips: Vec<String>,
    pub transport: TransportSpec,
    pub scenes: SceneSpec,
}

impl Profile {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != VERSION {
            return Err(format!("versión {} (esperaba {VERSION})", self.version));
        }
        if self.id.trim().is_empty() || self.name.trim().is_empty() {
            return Err("id y nombre no pueden estar vacíos".into());
        }
        if self.params.is_empty() {
            return Err("el perfil no declara parámetros".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for param in &self.params {
            if !seen.insert(param.id.as_str()) {
                return Err(format!("parámetro repetido {}", param.id));
            }
            if param.min > param.max {
                return Err(format!(
                    "{}: rango {}..{} invertido",
                    param.id, param.min, param.max
                ));
            }
            if !param.contains(param.default) {
                return Err(format!(
                    "{}: default {} fuera de rango",
                    param.id, param.default
                ));
            }
            match param.access {
                ParamAccess::Cc { channel, .. } | ParamAccess::Nrpn { channel, .. } => {
                    if !(1..=16).contains(&channel) {
                        return Err(format!("{}: canal {channel} fuera de 1..=16", param.id));
                    }
                }
                ParamAccess::Editor { .. } | ParamAccess::None => {}
            }
        }
        if self.default_strips.len() > 4 {
            return Err("default_strips admite hasta 4".into());
        }
        for id in &self.default_strips {
            if !self.params.iter().any(|param| &param.id == id) {
                return Err(format!("default_strip {id} no existe"));
            }
        }
        Ok(())
    }

    pub fn param(&self, id: &str) -> Option<&Param> {
        self.params.iter().find(|param| param.id == id)
    }

    /// El perfil que reconoce `identity` o `version` (el primero que matchea).
    pub fn matches<'a>(
        profiles: &'a [Profile],
        identity: &str,
        version: &str,
    ) -> Option<&'a Profile> {
        profiles.iter().find(|profile| {
            profile.matches.iter().any(|needle| {
                let needle = needle.to_lowercase();
                identity.to_lowercase().contains(&needle)
                    || version.to_lowercase().contains(&needle)
            })
        })
    }
}

/// Perfil genérico: mapa CC estándar del FM-1, seis canales de pista.
pub fn generic() -> Profile {
    let mut params = Vec::new();
    let mut default_strips = Vec::new();
    for channel in 1..=6u8 {
        for (suffix, label, cc, default) in [
            ("level", "LEVEL", 7u8, 104i16),
            ("pan", "PAN", 10, 64),
            ("cutoff", "CUTOFF", 74, 127),
            ("res", "RES", 71, 0),
            ("atk", "ATK", 73, 10),
            ("dec", "DEC", 75, 70),
            ("rel", "REL", 72, 60),
            ("glide", "GLIDE", 5, 0),
            ("rev", "REV", 91, 0),
            ("chorus", "CHORUS", 93, 0),
            ("delay", "DELAY", 94, 0),
        ] {
            let id = format!("t{channel}.{suffix}");
            if channel == 1 && matches!(suffix, "level" | "pan" | "cutoff" | "rev") {
                default_strips.push(id.clone());
            }
            params.push(Param::new(
                id,
                format!("T{channel} {label}"),
                Scope::Track,
                0,
                127,
                default,
                ParamAccess::Cc { channel, cc },
            ));
        }
    }
    Profile {
        version: VERSION,
        id: "generic".into(),
        name: "FM-1 genérico (CC estándar)".into(),
        matches: Vec::new(),
        params,
        default_strips,
        transport: TransportSpec::Midi,
        scenes: SceneSpec::None,
    }
}

/// Perfil de JIANT par12: el editor SysEx y su mapa de parámetros.
pub fn jiant() -> Profile {
    let mut params = Vec::new();
    for spec in nrpn::table(Scope::Global) {
        params.push(Param::new(
            format!("g.{}", spec.id),
            spec.name,
            Scope::Global,
            spec.min,
            spec.max,
            spec.default,
            ParamAccess::Editor {
                scope: Scope::Global,
                id: spec.id,
            },
        ));
    }
    // La tabla de pista repite ids para los valores EDIT de cada motor (mismo id, otro nombre). El
    // perfil se queda con la primera aparición (el parámetro base); los EDIT por motor son otra capa.
    let mut seen = std::collections::BTreeSet::new();
    for spec in nrpn::table(Scope::Track) {
        if !seen.insert(spec.id) {
            continue;
        }
        params.push(Param::new(
            format!("t.{}", spec.id),
            spec.name,
            Scope::Track,
            spec.min,
            spec.max,
            spec.default,
            ParamAccess::Editor {
                scope: Scope::Track,
                id: spec.id,
            },
        ));
    }
    Profile {
        version: VERSION,
        id: "jiant-par12".into(),
        name: "JIANT 0.9.9-par12".into(),
        matches: vec!["JIANT".into()],
        params,
        default_strips: vec!["t.5".into(), "g.17".into(), "g.8".into(), "g.7".into()],
        transport: TransportSpec::ClockMaster {
            clock: Some(ParamAccess::Editor {
                scope: Scope::Global,
                id: 2,
            }),
        },
        scenes: SceneSpec::ProgramChange {
            channel: 16,
            sections: vec![0, 1, 2, 3],
            projects: (16..22).collect(),
        },
    }
}

/// Perfil de X0X 1.2: 909 (canal 10), 808 (11), 303A (2), 303B (3), break (4) y MIX (1).
///
/// Mapa del manual (§16): 7 level, 10 pan, 91 reverb, 93 delay en cada parte; 16–19/20–23/24–27
/// en las 303; 20–30 TUNE, 70–80 DECAY y 102–112 LEVEL de las pistas de 909/808. X0X sigue el clock
/// externo solo, así que el transporte es `Midi`.
pub fn x0x() -> Profile {
    let mut params = Vec::new();
    let mut default_strips = Vec::new();

    let add = |id: String,
               label: String,
               channel: u8,
               cc: u8,
               min: i16,
               max: i16,
               default: i16,
               params: &mut Vec<Param>| {
        params.push(Param::new(
            id,
            label,
            Scope::Track,
            min,
            max,
            default,
            ParamAccess::Cc { channel, cc },
        ));
    };

    for (prefix, channel, tracks) in [
        (
            "909",
            10u8,
            [
                "BD", "SD", "LT", "MT", "HT", "RS", "CP", "CH", "OH", "CR", "RD",
            ],
        ),
        (
            "808",
            11u8,
            [
                "BD", "SD", "LT", "MT", "HT", "RS", "CP", "CB", "CY", "OH", "CH",
            ],
        ),
    ] {
        for (suffix, label, cc, default) in [
            ("level", "LEVEL", 7u8, 104i16),
            ("pan", "PAN", 10, 64),
            ("rev", "REV", 91, 0),
            ("dly", "DLY", 93, 0),
        ] {
            add(
                format!("{prefix}.{suffix}"),
                format!("{prefix} {label}"),
                channel,
                cc,
                0,
                127,
                default,
                &mut params,
            );
        }
        for (index, name) in tracks.iter().enumerate() {
            let track = name.to_lowercase();
            for (suffix, label, cc, default) in [
                ("tune", "TUNE", 20 + index as u8, 64i16),
                ("decay", "DECAY", 70 + index as u8, 64),
                ("level", "LEVEL", 102 + index as u8, 100),
            ] {
                add(
                    format!("{prefix}.{track}.{suffix}"),
                    format!("{prefix} {name} {label}"),
                    channel,
                    cc,
                    0,
                    127,
                    default,
                    &mut params,
                );
            }
        }
    }

    for (prefix, channel) in [("303a", 2u8), ("303b", 3u8)] {
        for (suffix, label, cc, default) in [
            ("level", "LEVEL", 7u8, 104i16),
            ("pan", "PAN", 10, 64),
            ("rev", "REV", 91, 0),
            ("dly", "DLY", 93, 0),
            ("cutoff", "CUTOFF", 16, 64),
            ("reso", "RESO", 17, 0),
            ("envmod", "ENVMOD", 18, 0),
            ("decay", "DECAY", 19, 64),
            ("accent", "ACCENT", 20, 0),
            ("wave", "WAVE", 21, 0),
            ("drive", "DRIVE", 22, 0),
            ("slide", "SLIDE", 23, 0),
            ("tune", "TUNE", 24, 64),
            ("volume", "VOLUME", 25, 100),
            ("drvtyp", "DRVTYP", 26, 0),
            ("accdec", "ACCDEC", 27, 64),
        ] {
            add(
                format!("{prefix}.{suffix}"),
                format!("{prefix} {label}"),
                channel,
                cc,
                0,
                127,
                default,
                &mut params,
            );
        }
    }

    for (suffix, label, cc, default) in [
        ("level", "LEVEL", 7u8, 104i16),
        ("pan", "PAN", 10, 64),
        ("rev", "REV", 91, 0),
        ("dly", "DLY", 93, 0),
        ("filter", "FILTER", 16, 64),
        ("pitch", "PITCH", 18, 64),
    ] {
        add(
            format!("break.{suffix}"),
            format!("BREAK {label}"),
            4,
            cc,
            0,
            127,
            default,
            &mut params,
        );
    }

    for id in ["909.level", "909.pan", "303a.cutoff", "303a.level"] {
        default_strips.push(id.to_string());
    }

    Profile {
        version: VERSION,
        id: "x0x-1.2".into(),
        name: "X0X 1.2.2".into(),
        matches: vec!["X0X".into(), "FM-1_9010202".into()],
        params,
        default_strips,
        transport: TransportSpec::Midi,
        scenes: SceneSpec::None,
    }
}

/// Todos los perfiles built-in, en orden de preferencia.
pub fn builtin() -> Vec<Profile> {
    vec![x0x(), jiant(), generic()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_generico_cubre_seis_pistas_y_tiene_defaults() {
        let profile = generic();
        profile.validate().unwrap();
        assert_eq!(profile.params.len(), 66);
        assert_eq!(profile.default_strips.len(), 4);
        let level = profile.param("t1.level").unwrap();
        assert_eq!(level.access, ParamAccess::Cc { channel: 1, cc: 7 });
        assert_eq!(
            profile.param("t6.delay").unwrap().access,
            ParamAccess::Cc { channel: 6, cc: 94 }
        );
        assert_eq!(profile.transport, TransportSpec::Midi);
        assert_eq!(profile.scenes, SceneSpec::None);
    }

    #[test]
    fn jiant_usa_el_editor_y_las_secciones() {
        let profile = jiant();
        profile.validate().unwrap();
        assert!(profile.param("t.5").is_some(), "FILTER de pista");
        assert!(profile.param("g.8").is_some(), "SIZE global");
        assert_eq!(
            profile.param("g.8").unwrap().access,
            ParamAccess::Editor {
                scope: Scope::Global,
                id: 8
            }
        );
        assert!(matches!(
            profile.transport,
            TransportSpec::ClockMaster { .. }
        ));
        assert!(matches!(
            profile.scenes,
            SceneSpec::ProgramChange { channel: 16, .. }
        ));
    }

    #[test]
    fn se_reconoce_por_identidad_o_version_y_si_no_cae_al_generico() {
        let profiles = builtin();
        assert_eq!(
            Profile::matches(&profiles, "FM-1_909", "JIANT v0.9.9-par12").map(|p| p.id.as_str()),
            Some("jiant-par12")
        );
        assert_eq!(
            Profile::matches(&profiles, "FM-1_9010202", "").map(|p| p.id.as_str()),
            Some("x0x-1.2")
        );
        assert_eq!(
            Profile::matches(&profiles, "X0X", "X0X 1.1.0").map(|p| p.id.as_str()),
            Some("x0x-1.2")
        );
        assert_eq!(
            Profile::matches(&profiles, "FOO", "BAR").map(|p| p.id.as_str()),
            None
        );
    }

    #[test]
    fn x0x_usa_los_canales_del_manual() {
        let profile = x0x();
        profile.validate().unwrap();
        assert_eq!(
            profile.param("909.level").unwrap().access,
            ParamAccess::Cc { channel: 10, cc: 7 }
        );
        assert_eq!(
            profile.param("303a.cutoff").unwrap().access,
            ParamAccess::Cc { channel: 2, cc: 16 }
        );
        assert_eq!(
            profile.param("808.bd.tune").unwrap().access,
            ParamAccess::Cc {
                channel: 11,
                cc: 20
            }
        );
        assert_eq!(profile.default_strips.len(), 4);
        assert_eq!(profile.transport, TransportSpec::Midi);
    }

    #[test]
    fn un_perfil_invalido_no_pasa() {
        let mut profile = generic();
        profile.version = 99;
        assert!(profile.validate().is_err());

        let mut profile = generic();
        profile.params.push(profile.params[0].clone());
        assert!(profile.validate().is_err());

        let mut profile = generic();
        profile.default_strips.push("t1.nope".into());
        assert!(profile.validate().is_err());
    }

    #[test]
    fn el_perfil_va_y_vuelve_por_json() {
        let profile = jiant();
        let json = serde_json::to_string(&profile).unwrap();
        let back: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile, back);
    }
}

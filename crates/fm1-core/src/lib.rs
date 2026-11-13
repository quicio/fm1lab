//! `fm1-core` — reglas del FM-1 sin infraestructura.
//!
//! Esta capa **no** abre puertos, **no** toca archivos y **no** conoce egui ni midir: sólo sabe qué
//! mensajes MIDI expresan una intención (`device`), qué parámetros existen (`nrpn`) y cómo se codifican
//! (`message`). La infraestructura vive en `fm1-midi` (adaptador midir) y en las apps.
//!
//! Regla de dependencia: `fm1-core` no depende de nadie de arriba. Si necesitás I/O acá, va detrás de
//! un trait de `transport` y la implementación queda afuera.

pub mod device;
pub mod editor;
pub mod message;
pub mod nrpn;
mod nrpn_table;
pub mod profile;
pub mod route;
pub mod setlist;
pub mod transport;

pub use device::Device;
pub use editor::Clock;
pub use nrpn::{EngineParamSpec, ParamSpec, Scope};
pub use profile::{Param, ParamAccess, Profile};
pub use setlist::{Item, Setlist};

/// Error de la capa de dominio. Nunca describe un puerto ni un archivo: eso es del adaptador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// El parámetro no existe en el mapa, o el valor está fuera de su rango.
    Param(String),
    /// El mensaje no cabe en el protocolo (canal fuera de 1..=16, etc.).
    Message(String),
    /// El aparato no contestó, o contestó algo que no entendemos.
    Transport(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Param(s) | Error::Message(s) | Error::Transport(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

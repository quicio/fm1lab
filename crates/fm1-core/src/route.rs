//! La ruta de instalación: qué paquete se puede escribir encima de qué.
//!
//! La instalación reemplaza la zona de aplicación de la flash (`0x4000`–`0x92FFF`) pero **no** la zona de
//! datos de arriba (presets y proyectos, alrededor de `0x97000` y `0xFC000`). Cada fork tiene su propio
//! formato ahí, así que escribir un fork encima de otro puede dejar datos viejos que el firmware nuevo lee
//! mal. El caso documentado es Felucca 0.9 → 1.0.x.
//!
//! La ruta “oficial en el medio” **no es ejecutable con este instalador**: el paquete de fábrica V16 no es
//! un paquete Felucca (no trae `FELUCCA-LOADER-1`), y el firmware que corre corta la lectura del paquete.
//! El stock sólo se instala con M-UPGRADE. Por eso, entre forks, la única ruta disponible es la directa, con
//! un aviso explícito: es una sola escritura y la zona de datos puede quedar incompatible.
//!
//! | desde                | a             | ruta                                  |
//! |----------------------|---------------|---------------------------------------|
//! | oficial de fábrica   | un fork       | directo                               |
//! | misma familia        | actualización | directo                               |
//! | un fork              | otro fork     | directo, con aviso de datos           |
//! | un fork              | oficial       | no: usar M-UPGRADE                    |
//! | modo update          | —             | no se escribe: se termina             |
//!
//! Es dominio puro: no abre puertos, no lee archivos, no sabe de SHA-256. La identidad del aparato y la del
//! paquete entran como texto; la ruta sale como una lista de pasos con su motivo.

use std::fmt;

/// De qué firmware viene un paquete o un aparato. `Other` es un fork que no conocemos: se lo trata como
/// fork, que es la decisión segura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Stock,
    Jiant,
    Dock,
    Sloop,
    Ctl,
    Felucca,
    Sidou,
    X0x,
    Other,
}

impl Family {
    /// El nombre que usa el catálogo (`firmwares.json`) y la CLI.
    pub fn slug(self) -> &'static str {
        match self {
            Family::Stock => "stock",
            Family::Jiant => "jiant",
            Family::Dock => "dock",
            Family::Sloop => "sloop",
            Family::Ctl => "ctl",
            Family::Felucca => "felucca",
            Family::Sidou => "sidou",
            Family::X0x => "x0x",
            Family::Other => "otro",
        }
    }

    /// Desde el nombre del catálogo. Lo que no está en la lista es `Other`: nunca se adivina una familia.
    pub fn parse(text: &str) -> Family {
        match text.trim().to_ascii_lowercase().as_str() {
            "stock" | "oficial" | "official" | "m-vave" => Family::Stock,
            "jiant" => Family::Jiant,
            "dock" | "fm1-dock" => Family::Dock,
            "sloop" => Family::Sloop,
            "ctl" | "ctl-1" => Family::Ctl,
            "felucca" => Family::Felucca,
            "sidou" => Family::Sidou,
            "x0x" => Family::X0x,
            _ => Family::Other,
        }
    }

    /// Desde la identidad que reporta el aparato por handshake.
    ///
    /// La identidad corta engaña: X0X contesta `FM-1_901` cuando se le pregunta con la forma corta y
    /// `FM-1_9010202` con el handshake largo. Por eso se compara también por prefijo, quedándose con el
    /// conocido más largo: sin eso, un aparato corriendo X0X parece un firmware desconocido.
    pub fn from_identity(identity: &str) -> Family {
        let text = identity.trim();
        if text.to_ascii_lowercase().starts_with("ota-") {
            // el loader de una actualización: no es una familia, es un estado (ver [`plan`])
            return Family::Other;
        }
        // La forma corta de X0X (`FM-1_901`) no es prefijo de nada conocido: se mapea explícita.
        if text == "FM-1_901" {
            return Family::X0x;
        }
        let known: &[(&str, Family)] = &[
            ("FM-1_014", Family::Stock),
            ("FM-1_015", Family::Stock),
            ("FM-1_016", Family::Stock),
            ("FM-1_909", Family::Jiant),
            ("FM-1_921", Family::Dock),
            ("FM-1_900", Family::Sloop),
            ("FM-1_918", Family::Felucca),
            ("FM-1_964", Family::Sidou),
            ("FM-1_6010", Family::Ctl),
            ("FM-1_9010", Family::X0x),
        ];
        known
            .iter()
            .filter(|(prefix, _)| text.starts_with(prefix))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, family)| *family)
            .unwrap_or(Family::Other)
    }
}

impl fmt::Display for Family {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

/// Lo que se quiere instalar.
#[derive(Debug, Clone)]
pub struct Target {
    /// Cómo se llama en la CLI y en el catálogo (`jiant-felucca-V10.7.23`, `x0x-1.2.2`).
    pub label: String,
    /// La identidad del paquete, leída de su encabezado (`FM-1_909`).
    pub identity: String,
    pub family: Family,
}

impl Target {
    pub fn new(label: impl Into<String>, identity: impl Into<String>, family: Family) -> Self {
        Self {
            label: label.into(),
            identity: identity.into(),
            family,
        }
    }
}

/// Qué paquete va en el paso. El llamador resuelve `Stock` a la entrada del catálogo y `Target` al paquete
/// que pidió el usuario; acá no hay rutas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    Stock,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanStep {
    pub kind: StepKind,
    /// La identidad que tiene que reportar el aparato cuando el paso termina.
    pub expect: String,
    pub reason: &'static str,
}

/// Por qué no se puede escribir. Ninguno de estos casos escribe nada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Hay una escritura sin terminar: primero se cierra esa, no se empieza otra.
    MidUpdate,
    /// Hace falta el paquete oficial y no está en el catálogo.
    NoStock,
    /// La ruta pasa por el oficial y eso resetea la zona de datos: hay que respaldar antes.
    NeedsBackup,
    /// El paquete oficial no es un paquete Felucca: se instala con M-UPGRADE, no por OTA.
    StockNeedsMUpgrade,
    /// El destino es el mismo estado que ya está corriendo.
    AlreadyThere(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::MidUpdate => f.write_str(
                "el aparato está en modo actualización: hay una escritura sin terminar.\n\
                 Terminá esa (volvé a correr el instalador con el mismo paquete) antes de empezar otra",
            ),
            Refusal::NoStock => f.write_str(
                "la ruta pasa por el firmware oficial y no hay ningún paquete `stock` en el catálogo:\n\
                 sumá el `FM-1.fwsc` de fábrica a firmwares.json (o a ~/.config/fm1lab/firmwares.json)",
            ),
            Refusal::NeedsBackup => f.write_str(
                "la ruta pasa por el oficial y eso resetea la zona de datos del firmware actual.\n\
                 Exportá tus proyectos primero (editor del firmware) y volvé a intentar",
            ),
            Refusal::StockNeedsMUpgrade => f.write_str(
                "el paquete oficial V16 no es un paquete Felucca: no se puede instalar por OTA.\n\
                 Para volver al stock usá M-UPGRADE; después fm1lab puede instalar otro fork",
            ),
            Refusal::AlreadyThere(identity) => {
                write!(f, "el aparato ya reporta {identity}: no hay nada que escribir")
            }
        }
    }
}

impl std::error::Error for Refusal {}

/// La ruta para llevar el aparato de `current` a `target`.
///
/// No hay paso intermedio por el oficial: su paquete V16 no es Felucca. Entre forks la ruta es directa y el
/// paso lleva el aviso de datos; el llamador decide si lo muestra y si pide confirmación.
pub fn plan(current: &str, target: &Target) -> Result<Vec<PlanStep>, Refusal> {
    let text = current.trim();
    if text.to_ascii_lowercase().starts_with("ota-") {
        return Err(Refusal::MidUpdate);
    }
    if text == target.identity {
        return Err(Refusal::AlreadyThere(text.to_string()));
    }
    let now = Family::from_identity(text);

    if now == target.family {
        return Ok(vec![PlanStep {
            kind: StepKind::Target,
            expect: target.identity.clone(),
            reason: "misma familia: actualización directa, la zona de datos se conserva",
        }]);
    }
    if target.family == Family::Stock {
        return Err(Refusal::StockNeedsMUpgrade);
    }
    if now == Family::Stock {
        return Ok(vec![PlanStep {
            kind: StepKind::Target,
            expect: target.identity.clone(),
            reason: "desde el oficial: directo",
        }]);
    }

    // fork → fork: no hay puente por el oficial (el stock no es Felucca); se escribe directo y se avisa.
    Ok(vec![PlanStep {
        kind: StepKind::Target,
        expect: target.identity.clone(),
        reason: "cambio directo entre forks: la zona de datos puede quedar incompatible; exportá los proyectos antes",
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jiant(identity: &str) -> Target {
        Target::new("jiant", identity, Family::Jiant)
    }

    #[test]
    fn misma_familia_es_un_solo_paso() {
        // el paquete se anuncia igual que el aparato (formato de identidad): se niega, no se reescribe
        assert!(matches!(
            plan("FM-1_909", &jiant("FM-1_909")),
            Err(Refusal::AlreadyThere(_))
        ));
        // misma familia, identidad distinta: un paso
        let steps = plan("FM-1_9091", &jiant("FM-1_909")).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].kind, StepKind::Target);
        assert!(steps[0].reason.contains("misma familia"));
    }

    #[test]
    fn fork_a_fork_es_directo_con_aviso() {
        let steps = plan("FM-1_9010202", &jiant("FM-1_909")).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].kind, StepKind::Target);
        assert_eq!(steps[0].expect, "FM-1_909");
        assert!(steps[0].reason.contains("directo entre forks"));
        assert!(steps[0].reason.contains("zona de datos"));
    }

    #[test]
    fn desde_el_oficial_es_directo() {
        let steps = plan("FM-1_016", &jiant("FM-1_909")).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].kind, StepKind::Target);
        assert!(steps[0].reason.contains("desde el oficial"));
    }

    #[test]
    fn volver_al_oficial_necesita_m_upgrade() {
        let stock = Target::new("stock", "FM-1_016", Family::Stock);
        assert_eq!(plan("FM-1_921", &stock), Err(Refusal::StockNeedsMUpgrade));
        // si ya está en el oficial, no hay nada que escribir
        assert!(matches!(
            plan("FM-1_016", &stock),
            Err(Refusal::AlreadyThere(_))
        ));
    }

    #[test]
    fn un_fork_desconocido_es_directo() {
        let steps = plan("FM-1_777", &jiant("FM-1_909")).unwrap();
        assert_eq!(
            steps.len(),
            1,
            "no se adivina: se escribe directo y se avisa"
        );
        assert_eq!(Family::from_identity("FM-1_777"), Family::Other);
    }

    #[test]
    fn en_modo_update_se_niega_siempre() {
        assert_eq!(
            plan("ota-FM-1_921", &jiant("FM-1_909")),
            Err(Refusal::MidUpdate)
        );
    }

    #[test]
    fn la_misma_identidad_no_se_reescribe() {
        assert_eq!(
            plan("FM-1_909", &jiant("FM-1_909")),
            Err(Refusal::AlreadyThere("FM-1_909".to_string()))
        );
    }

    #[test]
    fn la_identidad_corta_de_x0x_cae_en_su_familia() {
        // lo que contesta el handshake corto, y lo que contesta el largo
        assert_eq!(Family::from_identity("FM-1_901"), Family::X0x);
        assert_eq!(Family::from_identity("FM-1_9010202"), Family::X0x);
        // y no se confunde con SLOOP, que es 900
        assert_eq!(Family::from_identity("FM-1_900"), Family::Sloop);
    }

    #[test]
    fn las_familias_del_catalogo_se_leen_por_nombre() {
        assert_eq!(Family::parse("x0x"), Family::X0x);
        assert_eq!(Family::parse("fm1-dock"), Family::Dock);
        assert_eq!(Family::parse("oficial"), Family::Stock);
        assert_eq!(Family::parse("inventado"), Family::Other);
    }
}

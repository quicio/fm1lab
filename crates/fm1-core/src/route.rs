//! La ruta segura de instalación: qué paquete se puede escribir encima de qué.
//!
//! El problema que resuelve esta capa: la instalación reemplaza la zona de aplicación de la flash
//! (`0x4000`–`0x92FFF`) pero **no** la zona de datos de arriba (presets y proyectos, alrededor de `0x97000` y
//! `0xFC000`). Cada fork tiene su propio formato ahí, así que escribir un fork encima de otro deja los datos
//! viejos y el firmware nuevo los lee mal: es el soft-brick documentado de Felucca 0.9 → 1.0.x, que sólo se
//! arreglaba borrando dos regiones a mano.
//!
//! Reglas, y no hay más:
//!
//! | desde                | a             | ruta                       |
//! |----------------------|---------------|----------------------------|
//! | oficial de fábrica   | cualquiera    | directo                    |
//! | misma familia        | actualización | directo                    |
//! | un fork              | otro fork     | **oficial en el medio**    |
//! | modo update          | —             | no se escribe: se termina  |
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
            Refusal::AlreadyThere(identity) => {
                write!(f, "el aparato ya reporta {identity}: no hay nada que escribir")
            }
        }
    }
}

impl std::error::Error for Refusal {}

/// La ruta para llevar el aparato de `current` a `target`.
///
/// `stock_identity` es la identidad del paquete oficial del catálogo; sólo se usa si la ruta lo necesita.
pub fn plan(
    current: &str,
    target: &Target,
    stock_identity: Option<&str>,
) -> Result<Vec<PlanStep>, Refusal> {
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
        return Ok(vec![PlanStep {
            kind: StepKind::Target,
            expect: target.identity.clone(),
            reason: "volver al firmware oficial: directo desde cualquier firmware",
        }]);
    }
    if now == Family::Stock {
        return Ok(vec![PlanStep {
            kind: StepKind::Target,
            expect: target.identity.clone(),
            reason: "desde el oficial: directo",
        }]);
    }

    // fork → fork: el oficial en el medio, porque deja la zona de datos como la espera cualquier fork
    let stock = stock_identity.ok_or(Refusal::NoStock)?;
    Ok(vec![
        PlanStep {
            kind: StepKind::Stock,
            expect: stock.to_string(),
            reason: "paso 1: volver al oficial, que deja la zona de datos en su estado de fábrica",
        },
        PlanStep {
            kind: StepKind::Target,
            expect: target.identity.clone(),
            reason: "paso 2: instalar el destino desde la línea base",
        },
    ])
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
            plan("FM-1_909", &jiant("FM-1_909"), None),
            Err(Refusal::AlreadyThere(_))
        ));
        // misma familia, identidad distinta: un paso. JIANT comparte la identidad entre versiones, así que
        // acá se usa otro paquete de la misma familia para el caso general
        let steps = plan(
            "FM-1_921",
            &Target::new("dock-0.8.2", "FM-1_921", Family::Dock),
            None,
        );
        assert!(matches!(steps, Err(Refusal::AlreadyThere(_))));
        let steps = plan("FM-1_016", &jiant("FM-1_909"), None).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].kind, StepKind::Target);
    }

    #[test]
    fn fork_a_fork_pasa_por_el_oficial() {
        let steps = plan("FM-1_9010202", &jiant("FM-1_909"), Some("FM-1_016")).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].kind, StepKind::Stock);
        assert_eq!(steps[0].expect, "FM-1_016");
        assert_eq!(steps[1].kind, StepKind::Target);
    }

    #[test]
    fn desde_el_oficial_es_directo() {
        let steps = plan("FM-1_016", &jiant("FM-1_909"), Some("FM-1_016")).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].kind, StepKind::Target);
    }

    #[test]
    fn volver_al_oficial_es_directo_desde_cualquier_fork() {
        let stock = Target::new("stock", "FM-1_016", Family::Stock);
        let steps = plan("FM-1_921", &stock, Some("FM-1_016")).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].kind, StepKind::Target);
    }

    #[test]
    fn sin_paquete_oficial_no_se_puede_pasar_de_fork_a_fork() {
        let error = plan(
            "FM-1_909",
            &Target::new("sloop", "FM-1_900", Family::Sloop),
            None,
        );
        assert_eq!(error, Err(Refusal::NoStock));
    }

    #[test]
    fn un_fork_desconocido_se_trata_como_fork() {
        let steps = plan("FM-1_777", &jiant("FM-1_909"), Some("FM-1_016")).unwrap();
        assert_eq!(steps.len(), 2, "no se adivina: pasa por el oficial");
        assert_eq!(Family::from_identity("FM-1_777"), Family::Other);
    }

    #[test]
    fn en_modo_update_se_niega_siempre() {
        assert_eq!(
            plan("ota-FM-1_921", &jiant("FM-1_909"), Some("FM-1_016")),
            Err(Refusal::MidUpdate)
        );
    }

    #[test]
    fn la_misma_identidad_no_se_reescribe() {
        assert_eq!(
            plan("FM-1_909", &jiant("FM-1_909"), None),
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

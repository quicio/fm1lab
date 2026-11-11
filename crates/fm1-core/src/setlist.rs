//! El setlist: la unidad de trabajo de la app de performance.
//!
//! Es datos puros y serializables. Persistirlos es trabajo del adaptador de almacenamiento (JSON atómico),
//! no del dominio.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// Un paso del set: lo que se dispara al apretar una tecla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Item {
    /// Sección de la canción que está sonando (A–D).
    Section(char),
    /// Proyecto (1–6), con su sección. El firmware sólo lo carga con el transporte parado.
    Project { project: u8, section: char },
    /// Un sonido en una pista, de la lista PRESETS.
    Sound { track: u8, program: u8 },
}

impl Item {
    /// Cómo se muestra en una lista: corto, porque esto se lee en vivo.
    pub fn label(&self) -> String {
        match self {
            Item::Section(s) => format!("sección {}", s.to_ascii_uppercase()),
            Item::Project { project, section } => {
                format!("proyecto {project} · {}", section.to_ascii_uppercase())
            }
            Item::Sound { track, program } => format!("T{track} sonido {program}"),
        }
    }

    /// Valida los rangos que el aparato no va a validar por nosotros.
    pub fn validate(&self) -> Result<()> {
        match *self {
            Item::Section(s) => {
                if ('a'..='d').contains(&s.to_ascii_lowercase()) || ('A'..='D').contains(&s) {
                    Ok(())
                } else {
                    Err(Error::Param(format!("sección {s:?} inválida")))
                }
            }
            Item::Project { project, section } => {
                if !(1..=6).contains(&project) {
                    return Err(Error::Param(format!("proyecto {project} fuera de 1..=6")));
                }
                Item::Section(section).validate()
            }
            Item::Sound { track, program } => {
                if !(1..=6).contains(&track) {
                    return Err(Error::Param(format!("pista {track} fuera de 1..=6")));
                }
                if program > 127 {
                    return Err(Error::Param(format!("sonido {program} fuera de 0..=127")));
                }
                Ok(())
            }
        }
    }
}

/// Una lista ordenada de pasos, con nombre. El orden es el del show.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setlist {
    pub name: String,
    pub items: Vec<Item>,
}

impl Setlist {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            items: Vec::new(),
        }
    }

    pub fn push(&mut self, item: Item) -> Result<()> {
        item.validate()?;
        self.items.push(item);
        Ok(())
    }

    /// El paso `index`, si existe. El índice viene de la UI, así que se valida acá.
    pub fn get(&self, index: usize) -> Option<&Item> {
        self.items.get(index)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Valida todos los pasos: un setlist cargado de un archivo puede traer basura.
    pub fn validate(&self) -> Result<()> {
        self.items.iter().try_for_each(Item::validate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{Device, Section};
    use crate::transport::Recorder;

    #[test]
    fn un_setlist_de_un_show_real_serializa_y_vuelve() {
        let mut set = Setlist::new("viernes");
        set.push(Item::Project {
            project: 2,
            section: 'A',
        })
        .unwrap();
        set.push(Item::Section('B')).unwrap();
        set.push(Item::Sound {
            track: 3,
            program: 41,
        })
        .unwrap();

        let json = serde_json::to_string_pretty(&set).unwrap();
        let back: Setlist = serde_json::from_str(&json).unwrap();
        assert_eq!(set, back);
        assert!(json.contains("\"kind\": \"section\"") && json.contains("\"value\": \"B\""));
    }

    #[test]
    fn los_rangos_invalidos_se_rechazan_al_construir() {
        let mut set = Setlist::new("x");
        assert!(
            set.push(Item::Project {
                project: 9,
                section: 'A'
            })
            .is_err()
        );
        assert!(set.push(Item::Section('Z')).is_err());
        assert!(
            set.push(Item::Sound {
                track: 0,
                program: 1
            })
            .is_err()
        );
        assert!(set.is_empty());
    }

    #[test]
    fn disparar_un_paso_emite_lo_que_espera_el_aparato() {
        let mut device = Device::new(Recorder::default());
        device.play(&Item::Section('C')).unwrap();
        assert_eq!(device.out().last(), Some(&[0xCF, 0x02][..]));
    }

    #[test]
    fn un_proyecto_con_seccion_no_a_emite_dos_pasos() {
        let mut device = Device::new(Recorder::default());
        device
            .play(&Item::Project {
                project: 1,
                section: 'D',
            })
            .unwrap();
        assert_eq!(
            device.out().sent,
            vec![vec![0xCF, 0x10], vec![0xCF, 0x03]],
            "primero el proyecto, después la sección"
        );
    }

    #[test]
    fn la_etiqueta_es_corta_porque_se_lee_en_vivo() {
        assert_eq!(Item::Section('a').label(), "sección A");
        assert_eq!(
            Item::Project {
                project: 3,
                section: 'b'
            }
            .label(),
            "proyecto 3 · B"
        );
    }

    #[test]
    fn la_seccion_a_de_un_proyecto_no_necesita_salto_extra() {
        let mut device = Device::new(Recorder::default());
        device
            .play(&Item::Project {
                project: 1,
                section: 'A',
            })
            .unwrap();
        assert_eq!(device.out().sent.len(), 1);
        assert_eq!(Section::A.program(), 0);
    }
}

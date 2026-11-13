//! Catálogo de firmwares: built-in (`firmwares.json`, embebido) + el del usuario, y las operaciones de
//! disco que la app necesita para mostrar, verificar y respaldar antes de instalar.
//!
//! No habla MIDI ni decide si se flashea: sólo dice qué paquetes existen, dónde están, si su SHA-256
//! coincide y qué copia de seguridad se hizo.

use crate::sha256;
use crate::{Result, StoreError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// La versión del archivo de catálogo. Una distinta se rechaza.
pub const CATALOG_VERSION: u32 = 1;

/// El catálogo embebido viaja con el binario: la biblioteca funciona sin red y sin el repo.
const BUILTIN: &str = include_str!("../../../firmwares.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FirmwareEntry {
    /// Identificador estable (el que usa la UI y la CLI).
    pub id: String,
    pub name: String,
    pub version: String,
    /// Familia de firmware (`x0x`, `jiant`, `stock`, ...): sirve para avisar “otra versión instalada”.
    pub family: String,
    /// Identidad OTA que reporta al arrancar (p. ej. `FM-1_9010202`).
    pub identity: String,
    /// Nombre de archivo (relativo a `firmware_dir()`) o ruta absoluta.
    pub file: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: String,
    /// Perfil que conviene al instalar (`x0x-1.2`, `jiant-par12`, `generic`).
    #[serde(default)]
    pub profile: Option<String>,
    /// De dónde salió el paquete, para mostrarlo en la ficha.
    #[serde(default)]
    pub url: Option<String>,
}

impl FirmwareEntry {
    /// La ruta real del paquete: absoluta tal cual, o relativa a `firmware_dir()`.
    pub fn path(&self) -> PathBuf {
        let path = PathBuf::from(&self.file);
        if path.is_absolute() {
            path
        } else {
            firmware_dir().join(path)
        }
    }

    /// Texto para la lista: nombre, versión y etiquetas.
    pub fn subtitle(&self) -> String {
        let mut parts = vec![format!("v{}", self.version), self.family.clone()];
        parts.extend(self.tags.iter().cloned());
        parts.join(" · ")
    }

    fn validate(&self) -> std::result::Result<(), String> {
        for (field, value) in [
            ("id", &self.id),
            ("name", &self.name),
            ("version", &self.version),
            ("family", &self.family),
            ("identity", &self.identity),
            ("file", &self.file),
        ] {
            if value.trim().is_empty() {
                return Err(format!("entrada sin {field}"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    /// Está y el SHA-256 coincide (o la entrada no declara uno).
    Ready {
        path: PathBuf,
        sha256: String,
    },
    Missing {
        path: PathBuf,
    },
    Mismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    Unreadable {
        path: PathBuf,
        cause: String,
    },
}

impl FileState {
    pub fn is_ready(&self) -> bool {
        matches!(self, FileState::Ready { .. })
    }

    pub fn path(&self) -> &Path {
        match self {
            FileState::Ready { path, .. }
            | FileState::Missing { path }
            | FileState::Mismatch { path, .. }
            | FileState::Unreadable { path, .. } => path,
        }
    }

    /// Una línea corta para la UI y la CLI.
    pub fn label(&self) -> String {
        match self {
            FileState::Ready { .. } => "paquete listo · sha256 ok".into(),
            FileState::Missing { path } => format!("falta {}", path.display()),
            FileState::Mismatch {
                expected, actual, ..
            } => {
                format!("sha256 no coincide (esperaba {expected}, es {actual})")
            }
            FileState::Unreadable { path, cause } => {
                format!("no se pudo leer {}: {cause}", path.display())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Catalog {
    pub entries: Vec<FirmwareEntry>,
    /// Archivos de usuario que no se pudieron leer, con el motivo.
    pub errors: Vec<String>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self::builtin()
    }
}

impl Catalog {
    /// El catálogo embebido, sin tocar el disco del usuario.
    pub fn builtin() -> Self {
        let mut catalog = Self {
            entries: Vec::new(),
            errors: Vec::new(),
        };
        catalog.merge_bytes(BUILTIN.as_bytes(), "built-in");
        catalog
    }

    /// Built-in + `~/.config/fm1lab/firmwares.json` (el del usuario gana por `id`).
    pub fn load() -> Self {
        let mut catalog = Self::builtin();
        if let Ok(path) = catalog_path() {
            catalog.merge_path(&path);
        }
        catalog
    }

    /// Igual que [`Catalog::load`] pero con un archivo explícito (tests y `--catalog`).
    pub fn load_with(user: Option<&Path>) -> Self {
        let mut catalog = Self::builtin();
        if let Some(path) = user {
            catalog.merge_path(path);
        }
        catalog
    }

    /// Las entradas que matchean nombre, id, versión, familia, etiquetas o notas.
    pub fn search(&self, query: &str) -> Vec<&FirmwareEntry> {
        let needle = query.trim().to_lowercase();
        self.entries
            .iter()
            .filter(|entry| {
                needle.is_empty()
                    || entry.id.to_lowercase().contains(&needle)
                    || entry.name.to_lowercase().contains(&needle)
                    || entry.version.to_lowercase().contains(&needle)
                    || entry.family.to_lowercase().contains(&needle)
                    || entry.notes.to_lowercase().contains(&needle)
                    || entry
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&needle))
            })
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<&FirmwareEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    fn merge_path(&mut self, path: &Path) {
        match fs::read(path) {
            Ok(bytes) => self.merge_bytes(&bytes, &path.display().to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => self.errors.push(format!("{}: {error}", path.display())),
        }
    }

    fn merge_bytes(&mut self, bytes: &[u8], origin: &str) {
        let file = match serde_json::from_slice::<FileCatalog>(bytes) {
            Ok(file) => file,
            Err(error) => {
                self.errors.push(format!("{origin}: {error}"));
                return;
            }
        };
        if file.version != CATALOG_VERSION {
            self.errors.push(format!(
                "{origin}: catálogo versión {} (esperaba {CATALOG_VERSION})",
                file.version
            ));
            return;
        }
        for entry in file.entries {
            if let Err(error) = entry.validate() {
                self.errors.push(format!("{origin}: {}: {error}", entry.id));
                continue;
            }
            match self.entries.iter_mut().find(|known| known.id == entry.id) {
                Some(known) => *known = entry,
                None => self.entries.push(entry),
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct FileCatalog {
    version: u32,
    #[serde(default)]
    entries: Vec<FirmwareEntry>,
}

/// `FM1LAB_FIRMWARES`, si no `~/Proyectos/m-vave/firmware` cuando existe, si no
/// `~/.config/fm1lab/firmwares`.
pub fn firmware_dir() -> PathBuf {
    if let Some(value) = std::env::var_os("FM1LAB_FIRMWARES")
        && !value.is_empty()
    {
        return PathBuf::from(value);
    }
    if let Some(home) = std::env::var_os("HOME") {
        let repo = PathBuf::from(&home).join("Proyectos/m-vave/firmware");
        if repo.is_dir() {
            return repo;
        }
        return PathBuf::from(home)
            .join(".config")
            .join("fm1lab")
            .join("firmwares");
    }
    PathBuf::from("firmwares")
}

/// El catálogo de usuario: `FM1LAB_CATALOG` o `~/.config/fm1lab/firmwares.json`.
pub fn catalog_path() -> Result<PathBuf> {
    if let Some(value) = std::env::var_os("FM1LAB_CATALOG")
        && !value.is_empty()
    {
        return Ok(PathBuf::from(value));
    }
    Ok(config_dir()?.join("firmwares.json"))
}

/// La carpeta de configuración: `FM1LAB_CONFIG` o `$HOME/.config/fm1lab`.
pub fn config_dir() -> Result<PathBuf> {
    if let Some(value) = std::env::var_os("FM1LAB_CONFIG")
        && !value.is_empty()
    {
        return Ok(PathBuf::from(value));
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| StoreError::Io("no hay $HOME; fijá FM1LAB_CONFIG".into()))?;
    Ok(PathBuf::from(home).join(".config").join("fm1lab"))
}

/// Verifica un paquete contra el SHA-256 declarado.
pub fn state_of(entry: &FirmwareEntry) -> FileState {
    let path = entry.path();
    if !path.is_file() {
        return FileState::Missing { path };
    }
    match sha256::hash_file(&path) {
        Ok(actual) => {
            if entry.sha256.trim().is_empty() || entry.sha256.eq_ignore_ascii_case(&actual) {
                FileState::Ready {
                    path,
                    sha256: actual,
                }
            } else {
                FileState::Mismatch {
                    path,
                    expected: entry.sha256.clone(),
                    actual,
                }
            }
        }
        Err(error) => FileState::Unreadable {
            path,
            cause: error.to_string(),
        },
    }
}

/// Lo que se copió a `~/.config/fm1lab/backups/<fecha>/`.
#[derive(Debug, Clone)]
pub struct BackupReport {
    pub dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub errors: Vec<String>,
}

/// Respalda configuración y paquetes antes de una instalación.
///
/// Copia `setlist.json`, `panel.json`, `firmwares.json`, los perfiles de usuario y los `.fwsc` del
/// catálogo que existan. Es best-effort: si algo no se puede copiar queda en `errors`, pero el resto se
/// respalda igual.
pub fn backup_lab() -> Result<BackupReport> {
    let config = config_dir()?;
    let firmwares = firmware_dir();
    let profiles = crate::profiles_dir().unwrap_or_else(|_| config.join("profiles"));
    backup_lab_at(&config, &profiles, &firmwares, &Catalog::load())
}

/// Igual que [`backup_lab`] pero con rutas explícitas (tests y configuración forzada).
pub fn backup_lab_at(
    config: &Path,
    profiles: &Path,
    firmwares: &Path,
    catalog: &Catalog,
) -> Result<BackupReport> {
    let dir = config.join("backups").join(timestamp());
    fs::create_dir_all(dir.join("profiles")).map_err(|error| {
        StoreError::Io(format!(
            "no se pudo crear el backup {}: {error}",
            dir.display()
        ))
    })?;
    fs::create_dir_all(dir.join("firmwares")).map_err(|error| {
        StoreError::Io(format!(
            "no se pudo crear el backup {}: {error}",
            dir.display()
        ))
    })?;

    let mut report = BackupReport {
        dir: dir.clone(),
        files: Vec::new(),
        errors: Vec::new(),
    };

    for name in ["setlist.json", "panel.json", "firmwares.json"] {
        copy_into(&config.join(name), &dir, &mut report);
    }
    for entry in fs::read_dir(profiles).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            copy_into(&path, &dir.join("profiles"), &mut report);
        }
    }

    for entry in &catalog.entries {
        let path = catalog_entry_path(firmwares, &entry.file);
        if path.is_file() {
            copy_into(&path, &dir.join("firmwares"), &mut report);
        }
    }

    Ok(report)
}

fn catalog_entry_path(base: &Path, file: &str) -> PathBuf {
    let path = PathBuf::from(file);
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

fn copy_into(source: &Path, dest: &Path, report: &mut BackupReport) {
    if !source.is_file() {
        return;
    }
    let Some(name) = source.file_name() else {
        return;
    };
    let target = dest.join(name);
    match fs::copy(source, &target) {
        Ok(_) => report.files.push(target),
        Err(error) => report.errors.push(format!("{}: {error}", source.display())),
    }
}

/// `AAAAMMDD-HHMMSS` en UTC, sin dependencias de fecha.
pub fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let (year, month, day) = civil_from_days((secs / 86_400) as i64);
    let rem = secs % 86_400;
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rem / 3600,
        (rem / 60) % 60,
        rem % 60
    )
}

/// Días desde 1970-01-01 a fecha civil (algoritmo de Howard Hinnant, dominio público).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_catalogo_embebido_tiene_los_cuatro_firmwares_y_sha() {
        let catalog = Catalog::builtin();
        assert!(catalog.errors.is_empty(), "{:?}", catalog.errors);
        assert!(catalog.entries.len() >= 4);
        for entry in &catalog.entries {
            assert_eq!(entry.sha256.len(), 64, "{} sin sha256", entry.id);
            assert!(entry.identity.starts_with("FM-1_"), "{}", entry.id);
        }
        assert_eq!(
            catalog
                .get("x0x-1.2.2")
                .map(|entry| entry.identity.as_str()),
            Some("FM-1_9010202")
        );
    }

    #[test]
    fn la_busqueda_mira_nombre_tags_y_notas() {
        let catalog = Catalog::builtin();
        let acid: Vec<&str> = catalog
            .search("acid")
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert!(acid.iter().any(|id| id.contains("x0x")), "{acid:?}");
        assert!(
            catalog
                .search("jiant")
                .iter()
                .any(|entry| entry.family == "jiant")
        );
        assert!(catalog.search("").len() >= 4);
        assert!(catalog.search("no-existe").is_empty());
    }

    #[test]
    fn el_usuario_gana_por_id_y_los_rotos_se_avisan() {
        let dir = std::env::temp_dir().join(format!(
            "fm1-store-catalog-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("firmwares.json");
        fs::write(
            &path,
            br#"{"version":1,"entries":[
                {"id":"x0x-1.2.2","name":"X0X propio","version":"1.2.2","family":"x0x",
                 "identity":"FM-1_9010202","file":"x0x-1.2.2.fwsc","sha256":"00"},
                {"id":"roto","name":"","version":"1","family":"x","identity":"FM-1_x","file":"x"}
            ]}"#,
        )
        .unwrap();
        let catalog = Catalog::load_with(Some(&path));
        assert_eq!(catalog.get("x0x-1.2.2").unwrap().name, "X0X propio");
        assert!(catalog.errors.iter().any(|error| error.contains("roto")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_sha_que_no_coincide_no_esta_listo() {
        let dir = std::env::temp_dir().join(format!(
            "fm1-store-state-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("paquete.fwsc");
        fs::write(&path, b"hola").unwrap();
        let mut entry = FirmwareEntry {
            id: "test".into(),
            name: "Test".into(),
            version: "1".into(),
            family: "test".into(),
            identity: "FM-1_1".into(),
            file: path.display().to_string(),
            sha256: "00".into(),
            tags: Vec::new(),
            notes: String::new(),
            profile: None,
            url: None,
        };
        assert!(matches!(state_of(&entry), FileState::Mismatch { .. }));
        entry.sha256 = sha256::hex_digest(b"hola");
        assert!(state_of(&entry).is_ready());
        entry.file = dir.join("no-existe.fwsc").display().to_string();
        assert!(matches!(state_of(&entry), FileState::Missing { .. }));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_fecha_civil_sale_del_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_000), (2022, 1, 8));
        let stamp = timestamp();
        assert_eq!(stamp.len(), 15, "{stamp}");
        assert_eq!(&stamp[8..9], "-");
    }

    #[test]
    fn el_backup_copia_config_perfiles_y_paquetes() {
        let root = std::env::temp_dir().join(format!(
            "fm1-store-backup-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let config = root.join("config");
        let firmwares = root.join("firmwares");
        fs::create_dir_all(config.join("profiles")).unwrap();
        fs::create_dir_all(&firmwares).unwrap();
        fs::write(config.join("setlist.json"), b"{}").unwrap();
        fs::write(config.join("profiles/mio.json"), b"{}").unwrap();
        fs::write(firmwares.join("x0x-1.2.2.fwsc"), b"fw").unwrap();

        let report = backup_lab_at(
            &config,
            &config.join("profiles"),
            &firmwares,
            &Catalog::builtin(),
        )
        .unwrap();
        assert!(report.dir.starts_with(config.join("backups")));
        assert!(report.dir.join("setlist.json").is_file());
        assert!(report.dir.join("profiles/mio.json").is_file());
        assert!(report.dir.join("firmwares/x0x-1.2.2.fwsc").is_file());
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let _ = fs::remove_dir_all(&root);
    }
}

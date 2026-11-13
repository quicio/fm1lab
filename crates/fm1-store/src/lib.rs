//! Persistencia del setlist: JSON versionado y guardado atómico.
//!
//! Es un adaptador de disco: no sabe de MIDI ni de egui. `fm1-core` aporta el modelo y la validación;
//! acá se decide el formato, la ruta y cómo no perder el show si algo falla a mitad de escritura.

use fm1_core::{Item, Profile, Setlist};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

pub mod catalog;
pub mod sha256;

pub use catalog::{
    BackupReport, Catalog, FileState, FirmwareEntry, backup_lab, catalog_path, config_dir,
    firmware_dir, state_of,
};

/// Versión del archivo. Una versión distinta se rechaza hasta escribir su migración.
pub const VERSION: u32 = 1;

#[derive(Debug)]
pub enum StoreError {
    /// No hay archivo en esa ruta (la app usa el set del aparato).
    Missing(PathBuf),
    /// El archivo existe pero no se puede interpretar. `backup` es la copia que se dejó al costado.
    Corrupt {
        path: PathBuf,
        backup: Option<PathBuf>,
        cause: String,
    },
    /// El setlist no pasa la validación del dominio.
    Invalid(String),
    /// No se pudo leer o escribir.
    Io(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Missing(path) => write!(f, "no existe {}", path.display()),
            StoreError::Corrupt {
                path,
                backup,
                cause,
            } => {
                write!(f, "{} no se pudo leer: {cause}", path.display())?;
                if let Some(backup) = backup {
                    write!(f, " (copia en {})", backup.display())?;
                }
                Ok(())
            }
            StoreError::Invalid(cause) => write!(f, "setlist inválido: {cause}"),
            StoreError::Io(cause) => f.write_str(cause),
        }
    }
}

impl std::error::Error for StoreError {}

pub type Result<T> = std::result::Result<T, StoreError>;

/// El archivo: versión + el setlist tal como se serializa en `fm1-core`.
#[derive(Debug, Serialize, Deserialize)]
struct FileSetlist {
    version: u32,
    #[serde(flatten)]
    set: Setlist,
}

/// Lee y valida un setlist. Si el archivo está roto, lo mueve a `.bak` y devuelve el error.
pub fn load(path: &Path) -> Result<Setlist> {
    let bytes = fs::read(path).map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            StoreError::Missing(path.to_path_buf())
        } else {
            StoreError::Io(format!("no se pudo leer {}: {error}", path.display()))
        }
    })?;
    match parse(&bytes) {
        Ok(set) => Ok(set),
        Err(cause) => {
            let backup = backup(path);
            Err(StoreError::Corrupt {
                path: path.to_path_buf(),
                backup,
                cause,
            })
        }
    }
}

/// Guarda el setlist de forma atómica: temporal en el mismo directorio, `sync_all` y `rename`.
pub fn save(path: &Path, set: &Setlist) -> Result<()> {
    set.validate()
        .map_err(|error| StoreError::Invalid(error.to_string()))?;

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            StoreError::Io(format!(
                "no se pudo crear el directorio {}: {error}",
                parent.display()
            ))
        })?;
    }

    let file = FileSetlist {
        version: VERSION,
        set: set.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&file)
        .map_err(|error| StoreError::Io(format!("no se pudo serializar: {error}")))?;

    write_atomically(path, &bytes)
        .map_err(|error| StoreError::Io(format!("no se pudo guardar {}: {error}", path.display())))
}

/// La plantilla de fábrica: las cuatro secciones de un show vacío.
pub fn factory() -> Setlist {
    let mut set = Setlist::new("plantilla");
    for section in ['A', 'B', 'C', 'D'] {
        let _ = set.push(Item::Section(section));
    }
    set
}

/// La ruta del setlist: `FM1LAB_SETLIST` si está, si no `$HOME/.config/fm1lab/setlist.json`.
pub fn default_path() -> Result<PathBuf> {
    if let Some(value) = std::env::var_os("FM1LAB_SETLIST")
        && !value.is_empty()
    {
        return Ok(PathBuf::from(value));
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| StoreError::Io("no hay $HOME; fijá FM1LAB_SETLIST".into()))?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("fm1lab")
        .join("setlist.json"))
}

/// La carpeta de perfiles de usuario: `FM1LAB_PROFILES` o `$HOME/.config/fm1lab/profiles`.
pub fn profiles_dir() -> Result<PathBuf> {
    if let Some(value) = std::env::var_os("FM1LAB_PROFILES")
        && !value.is_empty()
    {
        return Ok(PathBuf::from(value));
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| StoreError::Io("no hay $HOME; fijá FM1LAB_PROFILES".into()))?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("fm1lab")
        .join("profiles"))
}

/// Perfiles built-in + los de usuario. Un perfil de usuario con el mismo id gana.
pub fn load_profiles() -> (Vec<Profile>, Vec<String>) {
    match profiles_dir() {
        Ok(dir) => load_profiles_from(&dir),
        Err(error) => (fm1_core::profile::builtin(), vec![error.to_string()]),
    }
}

/// Igual que [`load_profiles`] pero desde una carpeta explícita (tests y `--profiles`).
pub fn load_profiles_from(dir: &Path) -> (Vec<Profile>, Vec<String>) {
    let mut profiles = fm1_core::profile::builtin();
    let mut errors = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return (profiles, errors),
        Err(error) => {
            errors.push(format!("{}: {error}", dir.display()));
            return (profiles, errors);
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let parsed = fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                serde_json::from_slice::<Profile>(&bytes).map_err(|error| error.to_string())
            });
        match parsed {
            Ok(profile) => match profile.validate() {
                Ok(()) => {
                    profiles.retain(|built| built.id != profile.id);
                    profiles.insert(0, profile);
                }
                Err(error) => errors.push(format!("{}: {error}", path.display())),
            },
            Err(error) => errors.push(format!("{}: {error}", path.display())),
        }
    }
    (profiles, errors)
}

/// La ruta del panel: `FM1LAB_PANEL` o `$HOME/.config/fm1lab/panel.json`.
pub fn default_panel_path() -> Result<PathBuf> {
    if let Some(value) = std::env::var_os("FM1LAB_PANEL")
        && !value.is_empty()
    {
        return Ok(PathBuf::from(value));
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| StoreError::Io("no hay $HOME; fijá FM1LAB_PANEL".into()))?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("fm1lab")
        .join("panel.json"))
}

/// La ruta del último perfil elegido: `FM1LAB_LAST_PROFILE` o `$HOME/.config/fm1lab/profile.json`.
pub fn default_profile_path() -> Result<PathBuf> {
    if let Some(value) = std::env::var_os("FM1LAB_LAST_PROFILE")
        && !value.is_empty()
    {
        return Ok(PathBuf::from(value));
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| StoreError::Io("no hay $HOME; fijá FM1LAB_LAST_PROFILE".into()))?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("fm1lab")
        .join("profile.json"))
}

#[derive(Debug, Serialize, Deserialize)]
struct FileLastProfile {
    version: u32,
    id: String,
}

pub const LAST_PROFILE_VERSION: u32 = 1;

/// El perfil que el usuario eligió la última vez. `None` si no hay archivo.
pub fn load_last_profile(path: &Path) -> Result<Option<String>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(StoreError::Io(format!(
                "no se pudo leer {}: {error}",
                path.display()
            )));
        }
    };
    match serde_json::from_slice::<FileLastProfile>(&bytes) {
        Ok(file) if file.version == LAST_PROFILE_VERSION => {
            if file.id.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(file.id))
            }
        }
        Ok(file) => Err(StoreError::Invalid(format!(
            "último perfil versión {} (esperaba {LAST_PROFILE_VERSION})",
            file.version
        ))),
        Err(error) => {
            let backup = backup(path);
            Err(StoreError::Corrupt {
                path: path.to_path_buf(),
                backup,
                cause: error.to_string(),
            })
        }
    }
}

pub fn save_last_profile(path: &Path, id: &str) -> Result<()> {
    if id.trim().is_empty() {
        return Err(StoreError::Invalid("el perfil no puede estar vacío".into()));
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            StoreError::Io(format!(
                "no se pudo crear el directorio {}: {error}",
                parent.display()
            ))
        })?;
    }
    let file = FileLastProfile {
        version: LAST_PROFILE_VERSION,
        id: id.to_string(),
    };
    let bytes = serde_json::to_vec_pretty(&file)
        .map_err(|error| StoreError::Io(format!("no se pudo serializar: {error}")))?;
    write_atomically(path, &bytes)
        .map_err(|error| StoreError::Io(format!("no se pudo guardar {}: {error}", path.display())))
}

/// Las franjas por perfil: `profile_id -> ids de parámetros` (hasta 4).
pub type Panel = BTreeMap<String, Vec<String>>;

#[derive(Debug, Serialize, Deserialize)]
struct FilePanel {
    version: u32,
    #[serde(default)]
    profiles: Panel,
}

pub const PANEL_VERSION: u32 = 1;

pub fn load_panel(path: &Path) -> Result<Panel> {
    let bytes = fs::read(path).map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            StoreError::Missing(path.to_path_buf())
        } else {
            StoreError::Io(format!("no se pudo leer {}: {error}", path.display()))
        }
    })?;
    match serde_json::from_slice::<FilePanel>(&bytes) {
        Ok(file) if file.version == PANEL_VERSION => Ok(file.profiles),
        Ok(file) => Err(StoreError::Invalid(format!(
            "panel versión {} (esperaba {PANEL_VERSION})",
            file.version
        ))),
        Err(error) => {
            let backup = backup(path);
            Err(StoreError::Corrupt {
                path: path.to_path_buf(),
                backup,
                cause: error.to_string(),
            })
        }
    }
}

pub fn save_panel(path: &Path, panel: &Panel) -> Result<()> {
    for (profile, strips) in panel {
        if strips.len() > 4 {
            return Err(StoreError::Invalid(format!(
                "{profile}: {strips:?} tiene más de 4 franjas"
            )));
        }
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            StoreError::Io(format!(
                "no se pudo crear el directorio {}: {error}",
                parent.display()
            ))
        })?;
    }
    let file = FilePanel {
        version: PANEL_VERSION,
        profiles: panel.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&file)
        .map_err(|error| StoreError::Io(format!("no se pudo serializar: {error}")))?;
    write_atomically(path, &bytes)
        .map_err(|error| StoreError::Io(format!("no se pudo guardar {}: {error}", path.display())))
}

/// Guarda un perfil de usuario (lo que aprende el monitor) en la carpeta de perfiles.
pub fn save_profile(profile: &Profile) -> Result<PathBuf> {
    let dir = profiles_dir()?;
    save_profile_to(&dir, profile)
}

/// Igual que [`save_profile`] pero en una carpeta explícita (tests).
pub fn save_profile_to(dir: &Path, profile: &Profile) -> Result<PathBuf> {
    profile.validate().map_err(StoreError::Invalid)?;
    fs::create_dir_all(dir).map_err(|error| {
        StoreError::Io(format!(
            "no se pudo crear el directorio {}: {error}",
            dir.display()
        ))
    })?;
    let path = dir.join(format!("{}.json", sanitize(&profile.id)));
    let bytes = serde_json::to_vec_pretty(profile)
        .map_err(|error| StoreError::Io(format!("no se pudo serializar: {error}")))?;
    write_atomically(&path, &bytes).map_err(|error| {
        StoreError::Io(format!("no se pudo guardar {}: {error}", path.display()))
    })?;
    Ok(path)
}

fn sanitize(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// `setlist.json` → `setlist.json.tmp`.
pub fn tmp_path(path: &Path) -> PathBuf {
    path.with_file_name(format!("{}.tmp", name_of(path)))
}

/// `setlist.json` → `setlist.json.bak`.
pub fn backup_path(path: &Path) -> PathBuf {
    path.with_file_name(format!("{}.bak", name_of(path)))
}

fn parse(bytes: &[u8]) -> std::result::Result<Setlist, String> {
    let file: FileSetlist = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if file.version != VERSION {
        return Err(format!(
            "versión {} desconocida (este fm1lab escribe {VERSION})",
            file.version
        ));
    }
    file.set.validate().map_err(|error| error.to_string())?;
    Ok(file.set)
}

fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = tmp_path(path);
    let result = (|| {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn backup(path: &Path) -> Option<PathBuf> {
    let backup = backup_path(path);
    let _ = fs::remove_file(&backup);
    fs::rename(path, &backup).ok().map(|()| backup)
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "setlist.json".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fm1-store-{name}-{}-{nanos}.json",
            std::process::id()
        ))
    }

    fn twelve_steps() -> Setlist {
        let mut set = Setlist::new("viernes");
        for section in ['A', 'B', 'C', 'A'] {
            set.push(Item::Section(section)).unwrap();
        }
        set.push(Item::Project {
            project: 1,
            section: 'B',
        })
        .unwrap();
        set.push(Item::Section('D')).unwrap();
        set.push(Item::Sound {
            track: 3,
            program: 41,
        })
        .unwrap();
        for section in ['B', 'C', 'D', 'A', 'B'] {
            set.push(Item::Section(section)).unwrap();
        }
        assert_eq!(set.len(), 12);
        set
    }

    #[test]
    fn doce_pasos_van_y_vuelven_igual() {
        let path = temp("roundtrip");
        let set = twelve_steps();
        save(&path, &set).unwrap();
        let back = load(&path).unwrap();
        assert_eq!(set, back);
        assert!(!tmp_path(&path).exists(), "no queda temporal");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn la_plantilla_tiene_las_cuatro_secciones() {
        let path = temp("factory");
        let set = factory();
        save(&path, &set).unwrap();
        assert_eq!(load(&path).unwrap(), set);
        assert_eq!(set.name, "plantilla");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn un_archivo_ausente_es_missing_no_corrupto() {
        let path = temp("missing");
        assert!(matches!(load(&path), Err(StoreError::Missing(_))));
    }

    #[test]
    fn una_version_desconocida_se_rechaza_y_se_deja_bak() {
        let path = temp("version");
        fs::write(&path, br#"{"version":99,"name":"x","items":[]}"#).unwrap();
        let error = load(&path).unwrap_err();
        assert!(matches!(error, StoreError::Corrupt { .. }));
        assert!(!path.exists(), "el original se movió");
        assert!(backup_path(&path).exists(), "queda la copia");
        let _ = fs::remove_file(backup_path(&path));
    }

    #[test]
    fn un_paso_fuera_de_rango_no_se_carga() {
        let path = temp("invalid");
        fs::write(
            &path,
            br#"{"version":1,"name":"x","items":[{"kind":"section","value":"Z"}]}"#,
        )
        .unwrap();
        assert!(matches!(load(&path), Err(StoreError::Corrupt { .. })));
        assert!(backup_path(&path).exists());
        let _ = fs::remove_file(backup_path(&path));
    }

    #[test]
    fn un_setlist_invalido_no_se_guarda() {
        let path = temp("refuse");
        let mut set = Setlist::new("x");
        // Se construye a mano un paso inválido para saltar `push`.
        set.items.push(Item::Section('Z'));
        assert!(matches!(save(&path, &set), Err(StoreError::Invalid(_))));
        assert!(!path.exists());
    }

    #[test]
    fn las_rutas_de_trabajo_cuelgan_del_mismo_directorio() {
        let path = Path::new("/tmp/show/setlist.json");
        assert_eq!(tmp_path(path), Path::new("/tmp/show/setlist.json.tmp"));
        assert_eq!(backup_path(path), Path::new("/tmp/show/setlist.json.bak"));
    }

    #[test]
    fn los_perfiles_de_usuario_ganan_y_los_rotos_se_avisan() {
        let dir = std::env::temp_dir().join(format!(
            "fm1-store-profiles-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();

        let mut mine = fm1_core::profile::generic();
        mine.id = "mio".into();
        mine.name = "Perfil mío".into();
        fs::write(dir.join("mio.json"), serde_json::to_vec(&mine).unwrap()).unwrap();

        let mut override_generic = fm1_core::profile::generic();
        override_generic.name = "Genérico del usuario".into();
        fs::write(
            dir.join("generic.json"),
            serde_json::to_vec(&override_generic).unwrap(),
        )
        .unwrap();
        fs::write(dir.join("roto.json"), b"{ no es json").unwrap();

        let (profiles, errors) = load_profiles_from(&dir);
        assert!(profiles.iter().any(|profile| profile.id == "mio"));
        assert_eq!(
            profiles
                .iter()
                .find(|profile| profile.id == "generic")
                .unwrap()
                .name,
            "Genérico del usuario"
        );
        assert!(errors.iter().any(|error| error.contains("roto.json")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_panel_va_y_vuelve_y_respeta_cuatro_franjas() {
        let path = temp("panel");
        let mut panel = Panel::new();
        panel.insert(
            "jiant-par12".into(),
            vec!["t.5".into(), "g.17".into(), "g.8".into(), "g.7".into()],
        );
        save_panel(&path, &panel).unwrap();
        assert_eq!(load_panel(&path).unwrap(), panel);

        let mut too_many = Panel::new();
        too_many.insert(
            "x".into(),
            vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
        );
        assert!(matches!(
            save_panel(&path, &too_many),
            Err(StoreError::Invalid(_))
        ));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn el_ultimo_perfil_va_y_vuelve_y_ausente_es_none() {
        let path = temp("last-profile");
        assert_eq!(load_last_profile(&path).unwrap(), None);
        save_last_profile(&path, "x0x-1.2").unwrap();
        assert_eq!(
            load_last_profile(&path).unwrap().as_deref(),
            Some("x0x-1.2")
        );
        assert!(matches!(
            save_last_profile(&path, "  "),
            Err(StoreError::Invalid(_))
        ));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn un_perfil_aprendido_se_guarda_y_gana_al_builtin() {
        use fm1_core::Scope;
        use fm1_core::profile::{Param, ParamAccess};

        let dir = std::env::temp_dir().join(format!(
            "fm1-store-learn-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut profile = fm1_core::profile::generic();
        profile.params.push(Param::new(
            "cc.1.74",
            "CC 1.74",
            Scope::Track,
            0,
            127,
            0,
            ParamAccess::Cc { channel: 1, cc: 74 },
        ));
        let path = save_profile_to(&dir, &profile).unwrap();
        assert!(path.exists());
        let (profiles, errors) = load_profiles_from(&dir);
        assert!(errors.is_empty());
        let loaded = profiles
            .iter()
            .find(|profile| profile.id == "generic")
            .unwrap();
        assert!(loaded.param("cc.1.74").is_some());
        let _ = fs::remove_dir_all(&dir);
    }
}

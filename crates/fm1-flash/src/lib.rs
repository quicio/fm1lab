//! Instalador OTA/Felucca para el M-VAVE FM-1: lee un `.fwsc`, habla el handshake y responde los
//! pedidos de lectura del firmware/loader por USB-MIDI.
//!
//! Es el port del instalador probado de `m-vave` (`fm1_install.py`, GPL-3.0). El aparato arranca el
//! loader; el loader pide el **image lógico** (el paquete sin los 20 bytes de marcador) y nosotros
//! contestamos. No hay ACK de la escritura: la verificación es la identidad que reporta al reiniciar.
//!
//! Este crate no decide cuándo instalar. La app/CLI confirman, hacen backup y pausan otros clientes
//! MIDI antes de llamar a [`Installer::install`].

use fm1_core::message::IDENTITY_REQUEST;
use fm1_core::transport::{MidiIn, MidiOut};
use fm1_midi::Port;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

/// Pedido de identidad corta (`NAME_` + tres dígitos en firmwares viejos).
pub const HS_FULL: [u8; 12] = [
    0xF0, 0x00, 0x32, 0x45, 0x08, 0x00, 0x00, 0x40, 0x00, 0x7E, 0x01, 0xF7,
];
/// La orden de arrancar el actualizador.
pub const UPGRADE: [u8; 6] = [0xF0, 0x22, 0x24, 0x35, 0x7F, 0xF7];

const FINISH_CHECK: u32 = 0xE000_0000;
const FINISH_WRITE: u32 = 0xF000_0000;
const MAXDATA: usize = 512;
const BLOCKS: usize = 20;
const BLK: usize = 0x30;
const KEEP: usize = 0x2F;
const LOADER_MARK: &[u8] = b"FELUCCA-LOADER-1";

/// Los puertos que pueden ser un FM-1; nunca se prueba otro aparato.
const PORT_NAMES: [&str; 6] = ["fm-1", "felucca", "ota", "composite", "sinco", "usb-midi"];

#[derive(Debug, Clone, Copy)]
struct Timing {
    open: Duration,
    start: Duration,
    reply: Duration,
    loader: Duration,
    reboot: Duration,
    retry: Duration,
    poll: Duration,
    hs: Duration,
    hs_full: Duration,
    idle_check: Duration,
    idle_write: Duration,
    wait_loader: Duration,
    wait_reboot: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            open: Duration::from_millis(300),
            start: Duration::from_millis(2000),
            reply: Duration::from_millis(10),
            loader: Duration::from_millis(3000),
            reboot: Duration::from_millis(3000),
            retry: Duration::from_millis(1000),
            poll: Duration::from_millis(1000),
            hs: Duration::from_millis(1000),
            hs_full: Duration::from_millis(600),
            idle_check: Duration::from_millis(8000),
            idle_write: Duration::from_millis(180_000),
            wait_loader: Duration::from_millis(30_000),
            wait_reboot: Duration::from_millis(40_000),
        }
    }
}

/// Eventos para la UI/CLI. `Progress` trae los bytes servidos y el total del image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start(String),
    Loader,
    Progress {
        served: usize,
        end: usize,
        total: usize,
    },
    WriteDone,
    Reboot,
    Done(String),
}

#[derive(Debug)]
pub enum FlashError {
    BadPackage(String),
    NotFound(String),
    Model(String),
    Lost(String),
    Stopped(String),
    NoLoader(String),
    NoReturn(String),
    Mismatch(String),
    BadRequest(String),
    Io(String),
}

impl fmt::Display for FlashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlashError::BadPackage(s)
            | FlashError::NotFound(s)
            | FlashError::Model(s)
            | FlashError::Lost(s)
            | FlashError::Stopped(s)
            | FlashError::NoLoader(s)
            | FlashError::NoReturn(s)
            | FlashError::Mismatch(s)
            | FlashError::BadRequest(s)
            | FlashError::Io(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for FlashError {}

pub type Result<T> = std::result::Result<T, FlashError>;

/// Un paquete `.fwsc` ya validado y con su image lógico.
#[derive(Debug, Clone)]
pub struct Package {
    pub path: PathBuf,
    pub product: String,
    pub image: Vec<u8>,
}

impl Package {
    /// Lee y valida el paquete. `force` permite un paquete sin el loader de Felucca (no recomendado).
    pub fn load(path: &Path, force: bool) -> Result<Self> {
        let raw = fs::read(path).map_err(|error| {
            FlashError::BadPackage(format!("no se pudo leer {}: {error}", path.display()))
        })?;
        let product = product_of(&raw).ok_or_else(|| {
            FlashError::BadPackage(format!(
                "{}: no es un paquete FM-1 (muy corto)",
                path.display()
            ))
        })?;
        if !valid_product(&product) {
            return Err(FlashError::BadPackage(format!(
                "{}: identidad de paquete inválida {product:?}",
                path.display()
            )));
        }
        if !force
            && !raw
                .windows(LOADER_MARK.len())
                .any(|window| window == LOADER_MARK)
        {
            return Err(FlashError::BadPackage(format!(
                "{}: no tiene el loader de Felucca; no se instala sin --force",
                path.display()
            )));
        }
        Ok(Self {
            path: path.to_path_buf(),
            product,
            image: logical_image(&raw),
        })
    }
}

/// La identidad OTA del aparato, con su modelo y versión separados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub text: String,
    pub model: String,
    pub version: u32,
}

impl Identity {
    pub fn loader(&self) -> bool {
        self.model.to_lowercase().starts_with("ota-")
    }
}

struct Device {
    port: Port,
    identity: Identity,
    name: String,
}

/// El instalador. `port` es una subcadena opcional del puerto.
#[derive(Debug, Clone)]
pub struct Installer {
    port: Option<String>,
    timing: Timing,
}

impl Installer {
    pub fn new(port: Option<String>) -> Self {
        Self {
            port,
            timing: Timing::default(),
        }
    }

    /// Identidad y puerto del FM-1 conectado (sin escribir nada).
    pub fn probe(&self) -> Result<(Identity, String)> {
        let device = self.find(None, true).ok_or_else(|| self.not_found())?;
        Ok((device.identity, device.name))
    }

    /// Instala el paquete y verifica la identidad final.
    pub fn install(&self, package: &Package, progress: &mut dyn FnMut(Event)) -> Result<Identity> {
        let device = self.find(None, true).ok_or_else(|| self.not_found())?;
        if model_of(&device.identity.text) != model_of(&package.product) {
            return Err(FlashError::Model(format!(
                "el aparato es {}, el paquete es para {}",
                device.identity.text, package.product
            )));
        }
        let ota = if device.identity.loader() {
            device
        } else {
            self.check(device, &package.image, progress)?
        };
        self.write(ota, &package.image, progress)?;
        self.verify(&package.product, progress)
    }

    fn not_found(&self) -> FlashError {
        let names = fm1_midi::list()
            .map(|(ins, _)| ins.join(", "))
            .unwrap_or_else(|_| "(no se pudo listar)".into());
        let where_ = match &self.port {
            Some(port) => format!("ningún puerto que contenga {port:?} contestó"),
            None => "no se encontró el FM-1".into(),
        };
        FlashError::NotFound(format!(
            "{where_} (¿cable de datos? ¿otra app usando el puerto?)\n  entradas MIDI: {names}"
        ))
    }

    fn candidates(&self, names: &[String], only_port: bool) -> Vec<String> {
        if let Some(port) = &self.port {
            let lower = port.to_lowercase();
            let mine: Vec<String> = names
                .iter()
                .filter(|name| name.to_lowercase().contains(&lower))
                .cloned()
                .collect();
            if only_port {
                return mine;
            }
            let mut all = mine;
            for name in names {
                if !all.contains(name) && looks_like(name) {
                    all.push(name.clone());
                }
            }
            return all;
        }
        names
            .iter()
            .filter(|name| looks_like(name))
            .cloned()
            .collect()
    }

    fn find(&self, want: Option<&dyn Fn(&Identity) -> bool>, only_port: bool) -> Option<Device> {
        let (inputs, _) = fm1_midi::list().ok()?;
        for name in self.candidates(&inputs, only_port) {
            let Ok(mut port) = Port::open_matching(Some(&name)) else {
                continue;
            };
            thread::sleep(self.timing.open);
            if let Some(identity) = handshake(&mut port, &self.timing)
                && want.map(|want| want(&identity)).unwrap_or(true)
            {
                return Some(Device {
                    port,
                    identity,
                    name,
                });
            }
        }
        None
    }

    fn wait_for(&self, want: &dyn Fn(&Identity) -> bool, within: Duration) -> Option<Device> {
        let deadline = Instant::now() + within;
        while Instant::now() < deadline {
            if let Some(device) = self.find(Some(want), false) {
                return Some(device);
            }
            thread::sleep(self.timing.retry);
        }
        None
    }

    fn check(
        &self,
        mut device: Device,
        image: &[u8],
        progress: &mut dyn FnMut(Event),
    ) -> Result<Device> {
        progress(Event::Start(device.identity.text.clone()));
        let _ = device.port.send(&UPGRADE);
        thread::sleep(self.timing.start);
        let (served, finished, lost) = self.serve(
            &mut device.port,
            &device.name,
            image,
            FINISH_CHECK,
            self.timing.idle_check,
            progress,
        )?;
        drop(device.port);
        if !finished {
            return Err(if lost {
                FlashError::Lost(format!(
                    "el FM-1 se desconectó después de {served} pedidos; no se escribió nada"
                ))
            } else {
                FlashError::Stopped(format!(
                    "el FM-1 dejó de contestar después de {served} pedidos; no se escribió nada"
                ))
            });
        }
        progress(Event::Loader);
        thread::sleep(self.timing.loader);
        self.wait_for(&|identity: &Identity| identity.loader(), self.timing.wait_loader)
            .ok_or_else(|| {
                FlashError::NoLoader(
                    "el loader de actualización no apareció. Reenchufá el USB y repetí: el FM-1 queda en modo update"
                        .into(),
                )
            })
    }

    fn write(&self, mut ota: Device, image: &[u8], progress: &mut dyn FnMut(Event)) -> Result<()> {
        let _ = ota.port.send(&UPGRADE);
        thread::sleep(self.timing.start);
        let (served, finished, lost) = self.serve(
            &mut ota.port,
            &ota.name,
            image,
            FINISH_WRITE,
            self.timing.idle_write,
            progress,
        )?;
        drop(ota.port);
        if !finished {
            return Err(if lost {
                FlashError::Lost(format!(
                    "el loader se desconectó después de {served} pedidos; reenchufá y repetí para terminar"
                ))
            } else {
                FlashError::Stopped(format!(
                    "el loader dejó de contestar después de {served} pedidos; reenchufá y repetí para terminar"
                ))
            });
        }
        progress(Event::WriteDone);
        Ok(())
    }

    fn verify(&self, product: &str, progress: &mut dyn FnMut(Event)) -> Result<Identity> {
        progress(Event::Reboot);
        thread::sleep(self.timing.reboot);
        let back = self
            .wait_for(
                &|identity: &Identity| !identity.loader(),
                self.timing.wait_reboot,
            )
            .ok_or_else(|| FlashError::NoReturn("el FM-1 no volvió; apagalo y encendelo".into()))?;
        if back.identity.text != product {
            return Err(FlashError::Mismatch(format!(
                "se escribió, pero el FM-1 reporta {} (esperaba {product})",
                back.identity.text
            )));
        }
        progress(Event::Done(back.identity.text.clone()));
        Ok(back.identity)
    }

    #[allow(clippy::too_many_arguments)]
    fn serve(
        &self,
        port: &mut Port,
        name: &str,
        image: &[u8],
        finish: u32,
        idle: Duration,
        progress: &mut dyn FnMut(Event),
    ) -> Result<(usize, bool, bool)> {
        let mut served = 0;
        let mut last = Instant::now();
        loop {
            let Some(packet) = port.recv(self.timing.poll) else {
                if lost(name) {
                    return Ok((served, false, true));
                }
                if last.elapsed() > idle {
                    return Ok((served, false, false));
                }
                continue;
            };
            let Some((flags, addr, n)) = parse_request(&packet) else {
                continue;
            };
            last = Instant::now();
            if addr == FINISH_CHECK || addr == FINISH_WRITE {
                let _ = port.send(&response(addr, b"success\0", 0));
                if addr == finish {
                    return Ok((served, true, false));
                }
                continue;
            }
            if n > MAXDATA || addr as usize + n > image.len() {
                return Err(FlashError::BadRequest(format!(
                    "el aparato pidió {addr:#x}+{n}, fuera del paquete"
                )));
            }
            thread::sleep(self.timing.reply);
            if port
                .send(&response(
                    addr,
                    &image[addr as usize..addr as usize + n],
                    flags,
                ))
                .is_err()
            {
                return Ok((served, false, true));
            }
            served += 1;
            progress(Event::Progress {
                served,
                end: addr as usize + n,
                total: image.len(),
            });
        }
    }
}

fn looks_like(name: &str) -> bool {
    let lower = name.to_lowercase();
    PORT_NAMES.iter().any(|known| lower.contains(known))
}

fn lost(name: &str) -> bool {
    fm1_midi::list()
        .map(|(inputs, _)| !inputs.iter().any(|input| input == name))
        .unwrap_or(true)
}

fn handshake(port: &mut Port, timing: &Timing) -> Option<Identity> {
    drain(port);
    for _ in 0..3 {
        let Some(identity) = ask(port, &IDENTITY_REQUEST, timing.hs) else {
            continue;
        };
        if identity.loader() || !is_short(&identity.text) {
            return Some(identity);
        }
        if let Some(full) = ask(port, &HS_FULL, timing.hs_full)
            && full.model == identity.model
            && full.text.starts_with(&identity.text)
        {
            return Some(full);
        }
        return Some(identity);
    }
    None
}

fn ask(port: &mut Port, query: &[u8], within: Duration) -> Option<Identity> {
    port.send(query).ok()?;
    let deadline = Instant::now() + within;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return None;
        }
        let packet = port.recv(left)?;
        if let Some(identity) = parse_identity(&packet) {
            return Some(identity);
        }
    }
}

fn drain(port: &mut Port) {
    while port.recv(Duration::from_millis(0)).is_some() {}
}

fn is_short(text: &str) -> bool {
    text.rsplit_once('_')
        .map(|(_, version)| version.len() == 3 && version.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(false)
}

fn model_of(text: &str) -> &str {
    text.split('_')
        .next()
        .unwrap_or(text)
        .trim_start_matches("ota-")
}

fn valid_product(product: &str) -> bool {
    product
        .rsplit_once('_')
        .map(|(model, version)| {
            !model.is_empty() && !version.is_empty() && version.chars().all(|c| c.is_ascii_digit())
        })
        .unwrap_or(false)
}

fn parse_identity(packet: &[u8]) -> Option<Identity> {
    let text = fm1_core::message::parse_identity(packet)?
        .trim()
        .to_string();
    let (model, version) = text.rsplit_once('_')?;
    let version = version.parse::<u32>().ok()?;
    let model = model.to_string();
    Some(Identity {
        text,
        model,
        version,
    })
}

fn product_of(raw: &[u8]) -> Option<String> {
    if raw.len() < BLOCKS * BLK {
        return None;
    }
    let mut product = String::new();
    for i in 0..BLOCKS {
        let marker = raw[i * BLK + KEEP];
        if marker != 0x7D {
            product.push(((marker as i32 - i as i32 - 1) & 0xFF) as u8 as char);
        }
    }
    Some(product)
}

fn logical_image(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() - BLOCKS);
    for i in 0..BLOCKS {
        out.extend_from_slice(&raw[i * BLK..i * BLK + KEEP]);
    }
    out.extend_from_slice(&raw[BLOCKS * BLK..]);
    out
}

fn pack7(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 8 / 7 + 1);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for byte in data {
        acc |= (*byte as u32) << bits;
        bits += 8;
        while bits >= 7 {
            out.push((acc & 0x7F) as u8);
            acc >>= 7;
            bits -= 7;
        }
    }
    if bits > 0 {
        out.push((acc & 0x7F) as u8);
    }
    out
}

fn unpack7(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 7 / 8);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for byte in data {
        acc |= ((*byte & 0x7F) as u32) << bits;
        bits += 7;
        while bits >= 8 {
            out.push((acc & 0xFF) as u8);
            acc >>= 8;
            bits -= 8;
        }
    }
    out
}

fn response(addr: u32, data: &[u8], flags: u8) -> Vec<u8> {
    let n = data.len() as u16;
    let length = n + 8;
    let mut body = vec![
        0x00,
        0x59,
        0x30,
        (length & 0xFF) as u8,
        ((length >> 8) & 0xFF) as u8,
        0,
        flags,
    ];
    body.extend_from_slice(&addr.to_le_bytes());
    body.extend_from_slice(&[n as u8, (n >> 8) as u8, 0]);
    body.extend_from_slice(data);
    let checksum = !(body[6..].iter().map(|byte| *byte as u32).sum::<u32>() as u8);
    body.push(checksum);
    let mut packet = vec![0xF0];
    packet.extend_from_slice(&pack7(&body));
    packet.push(0xF7);
    packet
}

fn parse_request(packet: &[u8]) -> Option<(u8, u32, usize)> {
    if packet.len() < 2 || packet[0] != 0xF0 || packet[packet.len() - 1] != 0xF7 {
        return None;
    }
    let unpacked = unpack7(&packet[1..packet.len() - 1]);
    if unpacked.len() != 15 || unpacked[..3] != [0x00, 0x59, 0x30] {
        return None;
    }
    let checksum = !(unpacked[6..14].iter().map(|byte| *byte as u32).sum::<u32>() as u8);
    if checksum != unpacked[14] {
        return None;
    }
    let flags = unpacked[6];
    let addr = u32::from_le_bytes([unpacked[7], unpacked[8], unpacked[9], unpacked[10]]);
    let n = u16::from_le_bytes([unpacked[11], unpacked[12]]) as usize;
    Some((flags, addr, n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack7_y_unpack7_son_inversos() {
        let data: Vec<u8> = (0..=255).collect();
        assert_eq!(unpack7(&pack7(&data)), data);
        assert!(pack7(&data).iter().all(|byte| *byte < 0x80));
    }

    #[test]
    fn el_response_tiene_la_forma_del_contrato() {
        let packet = response(0x1234, &[1, 2, 3], 0);
        assert_eq!(packet[0], 0xF0);
        assert_eq!(packet[packet.len() - 1], 0xF7);
        let body = unpack7(&packet[1..packet.len() - 1]);
        assert_eq!(&body[..3], &[0x00, 0x59, 0x30]);
        assert_eq!(&body[7..11], &0x1234u32.to_le_bytes());
        assert_eq!(&body[11..14], &[3, 0, 0]);
        assert_eq!(&body[14..17], &[1, 2, 3]);
    }

    #[test]
    fn un_request_se_parsea_y_uno_roto_no() {
        let body = [
            0x00, 0x59, 0x30, 0x0F, 0x00, 0, 0, 0x34, 0x12, 0, 0, 3, 0, 0, 0,
        ];
        let mut body = body.to_vec();
        let checksum = !(body[6..14].iter().map(|byte| *byte as u32).sum::<u32>() as u8);
        body[14] = checksum;
        let mut packet = vec![0xF0];
        packet.extend_from_slice(&pack7(&body));
        packet.push(0xF7);
        assert_eq!(parse_request(&packet), Some((0, 0x1234, 3)));

        body[14] ^= 0xFF;
        let mut broken = vec![0xF0];
        broken.extend_from_slice(&pack7(&body));
        broken.push(0xF7);
        assert_eq!(parse_request(&broken), None);
    }

    #[test]
    fn la_identidad_separa_modelo_y_version() {
        let identity = Identity {
            text: "FM-1_9010202".into(),
            model: "FM-1".into(),
            version: 9010202,
        };
        assert!(!identity.loader());
        assert!(is_short("FM-1_909"));
        assert!(!is_short("FM-1_9010202"));
        assert_eq!(model_of("ota-FM-1_900"), "FM-1");
        assert_eq!(model_of("FM-1_909"), "FM-1");
    }

    #[test]
    fn un_paquete_corto_o_sin_loader_se_rechaza() {
        let path = std::env::temp_dir().join(format!("fm1-flash-test-{}.fwsc", std::process::id()));
        fs::write(&path, [0u8; 10]).unwrap();
        assert!(matches!(
            Package::load(&path, false),
            Err(FlashError::BadPackage(_))
        ));
        let _ = fs::remove_file(path);
    }
}

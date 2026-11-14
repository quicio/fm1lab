//! `fm1ctl` — el FM-1 sin abrir una ventana.
//!
//! Sirve para tres cosas: comprobar que el aparato está y quién es, mandar mensajes sueltos cuando algo
//! no anda, y correr un set sin UI. `--dry` imprime los bytes y no abre ningún puerto: es la forma de
//! verificar la codificación sin aparato (y lo que corre `scripts/check.sh`).

use fm1_core::device::{Device, Section};
use fm1_core::editor::{self, Clock};
use fm1_core::message;
use fm1_core::nrpn::{self, Scope};
use fm1_core::route::{self, Family, PlanStep, Refusal, StepKind, Target};
use fm1_core::transport::MidiOut;
use fm1_core::{Error, Result};
use std::path::Path;
use std::time::Duration;

const USAGE: &str = "\
uso: fm1ctl <comando> [args] [--dry] [--yes] [--force]

  ports                             puertos MIDI visibles
  info                              identidad del aparato (handshake de lectura)
  clock [int|usb|trs]               lee o pone la fuente de reloj (editor protocol)
  projects                          qué secciones del proyecto cargado tienen música
  song                              cadena de song: secciones y compases
  mix                               pista seleccionada y las cuatro franjas
  play [seg]                        arranca el transporte como reloj maestro y para
  setlist <archivo> [--dry]         valida y lista un setlist; --dry imprime los bytes
  setlist init <archivo>            escribe la plantilla A–D
  route <archivo|id>                la ruta segura para instalar, sin escribir nada
  flash <archivo> [puerto]          instala un .fwsc (necesita --yes para escribir)
  firmware [query]                  biblioteca de firmwares: estado del paquete y del aparato
  firmware --install <id>           instala una entrada del catálogo (necesita --yes)
  cc     <ch> <cc> <val>            control change
  pc     <ch> <prog>                program change (ch 16: 0-3 secciones, 16-21 proyectos)
  nrpn   <ch> <msb> <id> <val>      NRPN de 14 bits crudo (msb 0 pista / 1 global)
  bpm    <40..240>                  tempo del aparato
  section <A|B|C|D>                 salta de sección
  project <1..6> [sección]          carga un proyecto
  panic                             all sound off + all notes off en los 16 canales
  param  <global|track N> <nombre> <valor>
                                    p. ej.: param global BPM 140 · param track 1 FILTER 0
  listen [seg]                      imprime lo que manda el aparato
  params [global|track|engine M]    lista el mapa NRPN generado
  profile [id]                      perfiles disponibles, el detectado y el detalle

  --dry                             imprime los bytes/paquete en vez de enviarlos
  --yes                             confirma el flash (sin esto, no escribe)
  --force                           acepta un paquete sin loader de Felucca
  --straight                        saltea la ruta segura (fork sobre fork: puede no arrancar)

La ruta: misma familia o desde el oficial, directo; de fork a fork, el oficial en el medio.
Con el aparato en modo update no se escribe nada: se termina esa actualización primero.";

/// Las consultas del editor esperan entre frames de audio; 700 ms cubre una carga en curso.
const EDITOR_WAIT: Duration = Duration::from_millis(700);

fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let dry = args.iter().any(|a| a == "--dry");
    let yes = args.iter().any(|a| a == "--yes");
    let force = args.iter().any(|a| a == "--force");
    let straight = args.iter().any(|a| a == "--straight");
    args.retain(|a| a != "--dry" && a != "--yes" && a != "--force" && a != "--straight");

    let Some(command) = args.first().cloned() else {
        println!("{USAGE}");
        return Ok(());
    };
    let rest = &args[1..];

    match command.as_str() {
        "ports" => {
            let (ins, outs) = fm1_midi::list()?;
            for name in ins {
                println!("in : {name}");
            }
            for name in outs {
                println!("out: {name}");
            }
            Ok(())
        }
        "info" => {
            let mut port = fm1_midi::Port::open()?;
            println!("puerto: {}", port.name());
            match port.identify(Duration::from_millis(900)) {
                Some(identity) => println!("identidad: {identity}"),
                None => println!("identidad: (sin respuesta al handshake)"),
            }
            Ok(())
        }
        "clock" => clock(dry, rest),
        "projects" => projects(),
        "song" => song(),
        "mix" => mix(),
        "play" => play(rest),
        "setlist" => setlist(dry, rest),
        "route" => route_cmd(rest),
        "flash" => flash(dry, yes, force, straight, rest),
        "firmware" => firmware(dry, yes, force, straight, rest),
        "cc" => {
            let (channel, ctrl, value) = triple(rest)?;
            send(dry, &message::cc(channel, ctrl, value)?)
        }
        "pc" => {
            let (channel, program) = pair(rest)?;
            send(dry, &message::program_change(channel, program)?)
        }
        "nrpn" => {
            let [channel, msb, id, value] = quad(rest)?;
            let value = value as u16;
            let scope = if msb == 1 {
                Scope::Global
            } else {
                Scope::Track
            };
            for msg in message::nrpn(channel, scope, id, value)? {
                send(dry, &msg)?;
            }
            Ok(())
        }
        "bpm" => {
            let bpm = number(rest, 0)? as i16;
            with_device(dry, |device| device.set_bpm(bpm))
        }
        "section" => {
            let letter = rest.first().and_then(|s| s.chars().next()).unwrap_or('A');
            let section = Section::from_letter(letter)
                .ok_or_else(|| Error::Param(format!("sección {letter:?} inválida")))?;
            with_device(dry, |device| device.goto_section(section))
        }
        "project" => {
            let project = number(rest, 0)? as u8;
            let section = rest.get(1).and_then(|s| s.chars().next()).unwrap_or('A');
            let section = Section::from_letter(section)
                .ok_or_else(|| Error::Param(format!("sección {section:?} inválida")))?;
            with_device(dry, |device| {
                device.load_project(project)?;
                if section != Section::A {
                    device.goto_section(section)?;
                }
                Ok(())
            })
        }
        "panic" => with_device(dry, |device| device.panic()),
        "param" => {
            let scope = rest.first().cloned().unwrap_or_default();
            let (name, value) = match scope.as_str() {
                "global" => (
                    rest.get(1).cloned().unwrap_or_default(),
                    number(rest, 2)? as i16,
                ),
                "track" => {
                    let track = number(rest, 1)? as u8;
                    let name = rest.get(2).cloned().unwrap_or_default();
                    let value = number(rest, 3)? as i16;
                    return with_device(dry, |device| device.set_track(track, &name, value));
                }
                other => {
                    return Err(Error::Param(format!(
                        "ámbito {other:?}: esperaba `global` o `track N`"
                    )));
                }
            };
            with_device(dry, |device| device.set_global(&name, value))
        }
        "listen" => listen(rest),
        "params" => params(rest),
        "profile" => profile(rest),
        _ => {
            println!("{USAGE}");
            Err(Error::Message(format!("comando desconocido: {command}")))
        }
    }
}

/// Manda un mensaje: al aparato, o a la nada con los bytes impresos.
fn send(dry: bool, bytes: &[u8]) -> Result<()> {
    if dry {
        println!("  {}", hex(bytes));
        return Ok(());
    }
    let mut port = fm1_midi::Port::open()?;
    port.send(bytes)
}

/// Salida que imprime en vez de enviar: el `--dry` de todos los comandos de alto nivel.
#[derive(Debug, Default)]
struct Capture;

impl MidiOut for Capture {
    fn send(&mut self, message: &[u8]) -> Result<()> {
        println!("  {}", hex(message));
        Ok(())
    }
}

/// Corre una intención del dominio contra el aparato (o contra [`Capture`] con `--dry`).
fn with_device<F>(dry: bool, intent: F) -> Result<()>
where
    F: FnOnce(&mut Device<Box<dyn MidiOut>>) -> Result<()>,
{
    if dry {
        let mut device = Device::new(Box::new(Capture) as Box<dyn MidiOut>);
        return intent(&mut device);
    }
    let port = fm1_midi::Port::open()?;
    println!("puerto: {}", port.name());
    let mut device = Device::new(Box::new(port) as Box<dyn MidiOut>);
    intent(&mut device)?;
    println!("ok");
    Ok(())
}

/// Lee o pone la fuente de reloj. Sin argumento lee; con `int|usb|trs` escribe.
fn clock(dry: bool, rest: &[String]) -> Result<()> {
    let Some(name) = rest.first() else {
        return print_clock();
    };
    let clock = match name.to_ascii_lowercase().as_str() {
        "int" => Clock::Int,
        "usb" => Clock::Usb,
        "trs" => Clock::Trs,
        other => {
            return Err(Error::Param(format!(
                "reloj {other:?}: esperaba int, usb o trs"
            )));
        }
    };
    let request = editor::clock_set(clock);
    if dry {
        println!("  {}", hex(&request));
        return Ok(());
    }
    let mut port = fm1_midi::Port::open()?;
    println!("puerto: {}", port.name());
    let reply = port
        .editor(&request, EDITOR_WAIT)
        .ok_or_else(|| Error::Transport("el aparato no contestó al SET".into()))?;
    let (_, value) = reply
        .value()
        .ok_or_else(|| Error::Transport("respuesta inesperada al SET".into()))?;
    println!(
        "clock = {}",
        Clock::from_value(value).map(Clock::name).unwrap_or("?")
    );
    Ok(())
}

fn print_clock() -> Result<()> {
    let mut port = fm1_midi::Port::open()?;
    println!("puerto: {}", port.name());
    let reply = port
        .editor(&editor::clock_get(), EDITOR_WAIT)
        .ok_or_else(|| Error::Transport("el aparato no contestó al GET".into()))?;
    let (_, value) = reply
        .value()
        .ok_or_else(|| Error::Transport("respuesta inesperada al GET".into()))?;
    println!(
        "clock = {}",
        Clock::from_value(value).map(Clock::name).unwrap_or("?")
    );
    Ok(())
}

/// Las cuatro secciones del proyecto cargado y si tienen música.
fn projects() -> Result<()> {
    let mut port = fm1_midi::Port::open()?;
    println!("puerto: {}", port.name());
    for slot in 0..4u8 {
        let reply = port
            .editor(&editor::project_query(slot), EDITOR_WAIT)
            .ok_or_else(|| Error::Transport("el aparato no contestó a PROJECT".into()))?;
        let (_, used) = reply
            .project_used()
            .ok_or_else(|| Error::Transport("respuesta inesperada a PROJECT".into()))?;
        println!(
            "sección {}: {}",
            (b'A' + slot) as char,
            if used { "con música" } else { "vacía" }
        );
    }
    Ok(())
}

/// La cadena de song: qué secciones y cuántos compases.
fn song() -> Result<()> {
    let mut port = fm1_midi::Port::open()?;
    println!("puerto: {}", port.name());
    let chain = port
        .editor(&editor::song_query(), EDITOR_WAIT)
        .and_then(|reply| reply.song())
        .ok_or_else(|| Error::Transport("el aparato no contestó a SONG".into()))?;
    if chain.rows.is_empty() {
        println!("sin cadena configurada");
    } else {
        for (index, row) in chain.rows.iter().enumerate() {
            println!(
                "{:2}  sección {} · {} compases",
                index + 1,
                row.section,
                row.bars
            );
        }
    }
    println!("sonando: {}", if chain.running { "sí" } else { "no" });
    Ok(())
}

/// La pista seleccionada y los cuatro valores que muestra el panel.
fn mix() -> Result<()> {
    let mut port = fm1_midi::Port::open()?;
    let mix = port.mix()?;
    println!("pista: T{}", mix.selected + 1);
    println!("filtro: {}", mix.filter);
    println!("dj filter: {}", mix.dj_filter);
    println!("reverb size: {}", mix.size);
    println!("mix: {}", mix.mix);
    Ok(())
}

/// Arranca el transporte como reloj maestro por `seg` segundos y lo deja como estaba.
fn play(rest: &[String]) -> Result<()> {
    let secs = rest.first().and_then(|s| s.parse().ok()).unwrap_or(4_u64);
    let bpm = read_bpm()?;
    let master = fm1_midi::ClockMaster::start(f32::from(bpm))?;
    println!("play {secs}s a {bpm} BPM · CLOCK USB");
    std::thread::sleep(Duration::from_secs(secs));
    match master.stop() {
        Some(error) => Err(Error::Transport(error)),
        None => {
            println!("ok · reloj restaurado");
            Ok(())
        }
    }
}

fn read_bpm() -> Result<i16> {
    let mut port = fm1_midi::Port::open()?;
    let reply = port
        .editor(&editor::get(Scope::Global, editor::BPM_GLOBAL), EDITOR_WAIT)
        .ok_or_else(|| Error::Transport("el aparato no contestó al BPM".into()))?;
    let (_, value) = reply
        .value()
        .ok_or_else(|| Error::Transport("respuesta inesperada al BPM".into()))?;
    Ok(value.clamp(40, 240))
}

/// Valida y lista un setlist; con `--dry` imprime los bytes de cada paso.
///
/// `setlist init <archivo>` escribe la plantilla de fábrica. El archivo se lee completo y se valida
/// antes de mostrar nada: un paso fuera de rango no se dispara ni se imprime como si fuera bueno.
fn setlist(dry: bool, rest: &[String]) -> Result<()> {
    let Some(first) = rest.first() else {
        return Err(Error::Message(
            "setlist necesita <archivo> o `init <archivo>`".into(),
        ));
    };
    if first == "init" {
        let Some(path) = rest.get(1) else {
            return Err(Error::Message("init necesita <archivo>".into()));
        };
        let set = fm1_store::factory();
        fm1_store::save(Path::new(path), &set).map_err(store_error)?;
        println!("ok · {} pasos · {path}", set.len());
        return Ok(());
    }

    let set = fm1_store::load(Path::new(first)).map_err(store_error)?;
    println!("set: {} · {} pasos", set.name, set.len());
    let mut device = Device::new(Box::new(Capture) as Box<dyn MidiOut>);
    for (index, item) in set.items.iter().enumerate() {
        println!("{:02}  {}", index + 1, item.label());
        if dry {
            device
                .play(item)
                .map_err(|error| Error::Message(error.to_string()))?;
        }
    }
    if !dry {
        println!("validado · usá --dry para ver los bytes");
    }
    Ok(())
}

fn store_error(error: fm1_store::StoreError) -> Error {
    Error::Message(error.to_string())
}

/// El paquete oficial de fábrica, si está en el catálogo.
fn stock_package(catalog: &fm1_store::Catalog) -> Option<&fm1_store::FirmwareEntry> {
    catalog
        .search("")
        .into_iter()
        .find(|entry| Family::parse(&entry.family) == Family::Stock)
}

/// Si el cambio cruza de fork y por lo tanto puede mezclar la zona de datos.
fn crosses_forks(current: &str, target: &Target) -> bool {
    let now = Family::from_identity(current);
    now != target.family && now != Family::Stock && target.family != Family::Stock
}

fn print_plan(steps: &[PlanStep]) {
    println!(
        "ruta: {} paso{}",
        steps.len(),
        if steps.len() > 1 { "s" } else { "" }
    );
    for (index, step) in steps.iter().enumerate() {
        let what = match step.kind {
            StepKind::Stock => "oficial",
            StepKind::Target => "destino",
        };
        println!("  {}. {what} -> espera {}", index + 1, step.expect);
        println!("     {}", step.reason);
    }
}

fn print_event(event: fm1_flash::Event, line: &mut bool) {
    match event {
        fm1_flash::Event::Start(text) => println!("start: {text}"),
        fm1_flash::Event::Loader => println!("loader: modo update"),
        fm1_flash::Event::Progress { served, end, total } => {
            let percent = (end * 100 / total.max(1)).min(99);
            print!("\r  {served} lecturas · {percent}%");
            *line = true;
        }
        fm1_flash::Event::WriteDone => {
            println!();
            println!("  escritura completa");
            *line = false;
        }
        fm1_flash::Event::Reboot => println!("  reiniciando…"),
        fm1_flash::Event::Done(text) => println!("done: {text}"),
    }
}

/// La ruta que corresponde para este paquete, o el motivo por el que no se escribe.
fn plan_for(
    installer: &fm1_flash::Installer,
    target: &Target,
    straight: bool,
) -> Result<Vec<PlanStep>> {
    let (identity, port) = installer.probe().map_err(flash_error)?;
    println!("device: {} ({port})", identity.text);
    if straight {
        println!(
            "AVISO: --straight saltea la ruta segura. Un fork encima de otro puede dejar el FM-1 sin arrancar."
        );
        return Ok(vec![PlanStep {
            kind: StepKind::Target,
            expect: target.identity.clone(),
            reason: "sin ruta: se escribe directo (--straight)",
        }]);
    }
    route::plan(&identity.text, target)
        .map_err(|refusal: Refusal| Error::Message(refusal.to_string()))
}

/// Instala un `.fwsc` por el protocolo OTA, pasando antes por el oficial si cambia de familia.
/// `--dry` sólo lee y valida; `--yes` confirma la escritura.
fn flash(dry: bool, yes: bool, force: bool, straight: bool, rest: &[String]) -> Result<()> {
    let Some(path) = rest.first() else {
        return Err(Error::Message("flash necesita un .fwsc".into()));
    };
    let package = fm1_flash::Package::load(Path::new(path), force).map_err(flash_error)?;
    println!(
        "package: {} · {} bytes · {path}",
        package.product,
        package.image.len()
    );

    let target = Target::new(
        path.clone(),
        package.product.clone(),
        Family::from_identity(&package.product),
    );
    let installer = fm1_flash::Installer::new(rest.get(1).cloned());
    let steps = match plan_for(&installer, &target, straight) {
        Ok(steps) => steps,
        Err(error) if dry && !straight => {
            // con --dry, que no haya aparato no es un error: se valida el paquete y basta
            println!("plan: no se pudo leer el aparato ({error}); el paquete se validó");
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    print_plan(&steps);
    if dry {
        return Ok(());
    }
    if !yes {
        return Err(Error::Message(
            "flash escribe la flash; agregá --yes para confirmar".into(),
        ));
    }

    // Regla del lab: antes de escribir se respalda config y paquetes.
    match fm1_store::backup_lab() {
        Ok(report) => println!(
            "backup: {} · {} archivos",
            report.dir.display(),
            report.files.len()
        ),
        Err(error) => {
            return Err(Error::Message(format!(
                "no se pudo respaldar; no se escribió nada: {error}"
            )));
        }
    }

    let catalog = fm1_store::Catalog::load();
    let stock = stock_package(&catalog);
    for (index, step) in steps.iter().enumerate() {
        let step_path = match step.kind {
            StepKind::Stock => match stock {
                Some(entry) if fm1_store::state_of(entry).is_ready() => entry.path(),
                Some(entry) => {
                    return Err(Error::Message(format!(
                        "el paquete oficial no está listo: {}",
                        fm1_store::state_of(entry).label()
                    )));
                }
                None => return Err(Error::Message(Refusal::NoStock.to_string())),
            },
            StepKind::Target => Path::new(path).to_path_buf(),
        };
        println!("[{}/{}] {}", index + 1, steps.len(), step_path.display());
        let step_package =
            fm1_flash::Package::load(&step_path, force || step.kind == StepKind::Stock)
                .map_err(flash_error)?;
        let mut line = false;
        let written = installer
            .install(&step_package, &mut |event| print_event(event, &mut line))
            .map_err(flash_error)?;
        if line {
            println!();
        }
        if written.text != step.expect {
            return Err(Error::Message(format!(
                "después del paso {} el aparato dice {}, se esperaba {}",
                index + 1,
                written.text,
                step.expect
            )));
        }
    }
    if steps
        .iter()
        .any(|step| step.reason.contains("directo entre forks"))
    {
        println!("nota: la zona de datos del firmware anterior puede quedar incompatible");
    }
    println!("listo: el aparato corre {}", package.product);
    Ok(())
}

/// `route <archivo|id> [--from IDENT]`: la ruta, sin escribir nada.
fn route_cmd(rest: &[String]) -> Result<()> {
    let from = rest
        .iter()
        .position(|arg| arg == "--from")
        .and_then(|index| rest.get(index + 1))
        .cloned();
    let wanted = rest
        .iter()
        .enumerate()
        .find(|(index, arg)| {
            !arg.starts_with("--")
                && rest.get(index.wrapping_sub(1)).map(String::as_str) != Some("--from")
        })
        .map(|(_, arg)| arg.clone())
        .ok_or_else(|| Error::Message("route necesita un .fwsc o un id del catálogo".into()))?;

    let path = Path::new(&wanted);
    let target = if path.exists() {
        let package = fm1_flash::Package::load(path, false)
            .or_else(|_| fm1_flash::Package::load(path, true))
            .map_err(flash_error)?;
        Target::new(
            wanted.clone(),
            package.product.clone(),
            Family::from_identity(&package.product),
        )
    } else {
        let catalog = fm1_store::Catalog::load();
        let entry = catalog.get(&wanted).ok_or_else(|| {
            Error::Message(format!("{wanted} no es un archivo ni un id del catálogo"))
        })?;
        Target::new(
            entry.id.clone(),
            entry.identity.clone(),
            Family::parse(&entry.family),
        )
    };

    let current = match from {
        Some(identity) => identity,
        None => {
            fm1_flash::Installer::new(None)
                .probe()
                .map_err(flash_error)?
                .0
                .text
        }
    };
    println!("aparato: {current} ({})", Family::from_identity(&current));
    println!(
        "paquete: {} ({}, identidad {})",
        target.label, target.family, target.identity
    );
    let steps = route::plan(&current, &target)
        .map_err(|refusal: Refusal| Error::Message(refusal.to_string()))?;
    print_plan(&steps);
    if crosses_forks(&current, &target) {
        println!("se pierde: los proyectos y presets del firmware actual");
    }
    Ok(())
}

fn flash_error(error: fm1_flash::FlashError) -> Error {
    Error::Transport(error.to_string())
}

/// Perfiles disponibles, el que detecta el aparato y el detalle de uno pedido.
fn profile(rest: &[String]) -> Result<()> {
    let (profiles, errors) = fm1_store::load_profiles();
    for error in errors {
        eprintln!("perfil: {error}");
    }
    let detected = fm1_midi::detect_profile(&profiles).ok();

    if let Some(id) = rest.first() {
        let Some(entry) = profiles.iter().find(|profile| &profile.id == id) else {
            return Err(Error::Message(format!("no existe el perfil {id}")));
        };
        println!("{} · {}", entry.id, entry.name);
        println!("parámetros: {}", entry.params.len());
        println!("transporte: {:?}", entry.transport);
        println!("escenas: {:?}", entry.scenes);
        println!(
            "franjas por defecto: {}",
            if entry.default_strips.is_empty() {
                "(ninguna)".to_string()
            } else {
                entry.default_strips.join(" · ")
            }
        );
        for param in entry.params.iter().take(20) {
            println!(
                "  {:<16} {:<18} {:>4}..{:<4} {:?}",
                param.id, param.label, param.min, param.max, param.access
            );
        }
        if entry.params.len() > 20 {
            println!("  … {} más", entry.params.len() - 20);
        }
        return Ok(());
    }

    match &detected {
        Some(profile) => println!("detectado: {} · {}", profile.id, profile.name),
        None => println!("detectado: sin respuesta (¿aparato conectado?)"),
    }
    println!("\n{} perfiles:", profiles.len());
    for entry in &profiles {
        let marker = if detected
            .as_ref()
            .is_some_and(|profile| profile.id == entry.id)
        {
            "*"
        } else {
            " "
        };
        println!(
            "{marker} {:<16} {:<26} {:>3} params  {}",
            entry.id,
            entry.name,
            entry.params.len(),
            transport_name(&entry.transport)
        );
    }
    Ok(())
}

fn transport_name(spec: &fm1_core::profile::TransportSpec) -> &'static str {
    match spec {
        fm1_core::profile::TransportSpec::ClockMaster { .. } => "clock master",
        fm1_core::profile::TransportSpec::Midi => "clock MIDI",
        fm1_core::profile::TransportSpec::None => "sin transporte",
    }
}

/// La biblioteca desde la CLI: lista con el estado del paquete, o instala una entrada por `id`.
fn firmware(dry: bool, yes: bool, force: bool, straight: bool, rest: &[String]) -> Result<()> {
    let catalog = fm1_store::Catalog::load();
    for error in &catalog.errors {
        eprintln!("catálogo: {error}");
    }

    let install_id = rest
        .iter()
        .position(|arg| arg == "--install")
        .and_then(|index| rest.get(index + 1))
        .cloned();

    if let Some(id) = install_id {
        let Some(entry) = catalog.get(&id) else {
            return Err(Error::Message(format!("no existe el firmware {id}")));
        };
        let state = fm1_store::state_of(entry);
        println!("{} · {}", entry.name, entry.subtitle());
        println!("identidad: {}", entry.identity);
        println!("paquete: {}", state.label());
        if !state.is_ready() {
            return Err(Error::Message(
                "el paquete no está listo: revisá ruta y sha256".into(),
            ));
        }
        let path = entry.path().display().to_string();
        return flash(dry, yes, force, straight, &[path]);
    }

    let query = rest
        .iter()
        .filter(|arg| !arg.starts_with("--"))
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let installed = fm1_flash::Installer::new(None)
        .probe()
        .ok()
        .map(|(identity, port)| (identity.text, port));
    match &installed {
        Some((identity, port)) => println!("aparato: {identity} ({port})"),
        None => println!("aparato: sin respuesta (¿conectado? ¿otra app usando el puerto?)"),
    }

    let entries = catalog.search(&query);
    println!(
        "\n{} firmware(s){} · dir {}",
        entries.len(),
        if query.is_empty() {
            String::new()
        } else {
            format!(" para «{query}»")
        },
        fm1_store::firmware_dir().display()
    );
    for entry in entries {
        let marker = if installed
            .as_ref()
            .is_some_and(|(identity, _)| identity == &entry.identity)
        {
            "*"
        } else {
            " "
        };
        let state = fm1_store::state_of(entry);
        let state_text = if state.is_ready() {
            "listo".to_string()
        } else {
            state.label()
        };
        println!(
            "{marker} {:<16} {:<22} v{:<7} {:<6} {:<14} {}",
            entry.id, entry.name, entry.version, entry.family, entry.identity, state_text
        );
        if !entry.tags.is_empty() {
            println!("    {}", entry.tags.join(" · "));
        }
    }
    Ok(())
}

fn listen(rest: &[String]) -> Result<()> {
    use fm1_core::transport::MidiIn;
    let secs = rest.first().and_then(|s| s.parse().ok()).unwrap_or(10);
    let mut port = fm1_midi::Port::open()?;
    println!("escuchando {secs}s en {}…", port.name());
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    while std::time::Instant::now() < deadline {
        if let Some(message) = port.recv(Duration::from_millis(300)) {
            println!("{:>11} | {}", kind(&message), hex(&message));
        }
    }
    Ok(())
}

fn params(rest: &[String]) -> Result<()> {
    match rest.first().map(String::as_str) {
        Some("global") | None => {
            println!("globales ({}):", nrpn::count(Scope::Global));
            for p in nrpn::table(Scope::Global) {
                println!(
                    "  {:>3} {:<10} {:>4}..{:<4} (def {})",
                    p.id, p.name, p.min, p.max, p.default
                );
            }
        }
        Some("track") => {
            println!("pistas ({}):", nrpn::count(Scope::Track));
            for p in nrpn::table(Scope::Track) {
                println!(
                    "  {:>3} {:<10} {:>4}..{:<4} (def {})",
                    p.id, p.name, p.min, p.max, p.default
                );
            }
        }
        Some("engine") => {
            let engine = rest.get(1).cloned().unwrap_or_else(|| "ANALOG".into());
            for p in nrpn::engine_params(&engine) {
                println!("  {:>3} {:<10} {:>4}..{:<4}", p.id, p.name, p.min, p.max);
            }
        }
        Some(other) => return Err(Error::Param(format!("no conozco {other:?}"))),
    }
    Ok(())
}

fn kind(message: &[u8]) -> &'static str {
    match message.first().map(|b| b & 0xF0) {
        Some(0x90) => "note-on",
        Some(0x80) => "note-off",
        Some(0xB0) => "cc",
        Some(0xC0) => "pc",
        Some(0xE0) => "bend",
        Some(0xD0) => "aftertouch",
        Some(0xF0) => "sysex",
        _ => "?",
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn number(rest: &[String], index: usize) -> Result<u16> {
    rest.get(index)
        .ok_or_else(|| Error::Message(format!("falta el argumento {}", index + 1)))?
        .parse()
        .map_err(|_| Error::Message(format!("no es un número: {:?}", rest[index])))
}

fn pair(rest: &[String]) -> Result<(u8, u8)> {
    Ok((number(rest, 0)? as u8, number(rest, 1)? as u8))
}

fn triple(rest: &[String]) -> Result<(u8, u8, u8)> {
    Ok((
        number(rest, 0)? as u8,
        number(rest, 1)? as u8,
        number(rest, 2)? as u8,
    ))
}

fn quad(rest: &[String]) -> Result<[u8; 4]> {
    Ok([
        number(rest, 0)? as u8,
        number(rest, 1)? as u8,
        number(rest, 2)? as u8,
        number(rest, 3)? as u8,
    ])
}

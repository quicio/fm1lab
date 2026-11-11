//! `fm1ctl` — el FM-1 sin abrir una ventana.
//!
//! Sirve para tres cosas: comprobar que el aparato está y quién es, mandar mensajes sueltos cuando algo
//! no anda, y correr un set sin UI. `--dry` imprime los bytes y no abre ningún puerto: es la forma de
//! verificar la codificación sin aparato (y lo que corre `scripts/check.sh`).

use fm1_core::device::{Device, Section};
use fm1_core::message;
use fm1_core::nrpn::{self, Scope};
use fm1_core::transport::MidiOut;
use fm1_core::{Error, Result};
use std::time::Duration;

const USAGE: &str = "\
uso: fm1ctl <comando> [args] [--dry]

  ports                             puertos MIDI visibles
  info                              identidad del aparato (handshake de lectura)
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

  --dry                             imprime los bytes en vez de enviarlos";

fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let dry = args.iter().any(|a| a == "--dry");
    args.retain(|a| a != "--dry");

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

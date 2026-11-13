//! `fm1-perf` — el panel de performance del M-VAVE FM-1.
//!
//! Sin webview a propósito (ADR-0002): la ventana dibuja con egui y cada tecla sale como byte MIDI en el
//! mismo frame. `--shot` guarda una captura y cierra: es la forma de verificar el dibujo sin mirar.

mod app;
mod library;
mod link;
mod panels;
mod profiles;
mod theme;
mod widgets;

use app::PerfApp;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const USAGE: &str = "\
uso: fm1-perf [--shot <archivo.png>] [--demo] [--read] [--mix <seg>] [--setlist <archivo>] [--play-close <seg>] [--profile <id>]

  --shot <archivo.png>   dibuja un frame, guarda la captura y sale (verificación sin ojos)
  --demo                 siembra el historial con envíos de ejemplo (para capturas y demos)
  --read                 lee el aparato y imprime el set que armaría, sin abrir la ventana
  --mix <seg>            sigue las franjas por <seg> segundos y las imprime al cambiar
  --setlist <archivo>    usa ese setlist en vez de ~/.config/fm1lab/setlist.json
  --play-close <seg>     arranca el reloj y cierra solo (prueba de restauración)
  --profile <id>         usa ese perfil en vez de detectarlo
  --library              abre la biblioteca de firmwares al arrancar
  --firmware <id>        abre la biblioteca con esa entrada seleccionada
  --profiles             abre el selector de perfil al arrancar
";

fn main() -> eframe::Result<()> {
    let mut shot: Option<PathBuf> = None;
    let mut demo = false;
    let mut read = false;
    let mut mix: Option<u64> = None;
    let mut play_close: Option<u64> = None;
    let mut setlist: Option<PathBuf> = None;
    let mut profile: Option<String> = None;
    let mut library = false;
    let mut profiles_open = false;
    let mut firmware: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--shot" => match args.next() {
                Some(path) => shot = Some(PathBuf::from(path)),
                None => {
                    eprintln!("--shot necesita un archivo\n\n{USAGE}");
                    return Ok(());
                }
            },
            "--mix" => match args.next().and_then(|value| value.parse().ok()) {
                Some(secs) => mix = Some(secs),
                None => {
                    eprintln!("--mix necesita segundos\n\n{USAGE}");
                    return Ok(());
                }
            },
            "--setlist" => match args.next() {
                Some(path) => setlist = Some(PathBuf::from(path)),
                None => {
                    eprintln!("--setlist necesita un archivo\n\n{USAGE}");
                    return Ok(());
                }
            },
            "--profile" => match args.next() {
                Some(id) => profile = Some(id),
                None => {
                    eprintln!("--profile necesita un id\n\n{USAGE}");
                    return Ok(());
                }
            },
            "--play-close" => match args.next().and_then(|value| value.parse().ok()) {
                Some(secs) => play_close = Some(secs),
                None => {
                    eprintln!("--play-close necesita segundos\n\n{USAGE}");
                    return Ok(());
                }
            },
            "--demo" => demo = true,
            "--read" => read = true,
            "--library" => library = true,
            "--profiles" => profiles_open = true,
            "--firmware" => match args.next() {
                Some(id) => firmware = Some(id),
                None => {
                    eprintln!("--firmware necesita un id\n\n{USAGE}");
                    return Ok(());
                }
            },
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => {
                eprintln!("no conozco {other:?}\n\n{USAGE}");
                return Ok(());
            }
        }
    }

    if read {
        let (profiles, errors) = fm1_store::load_profiles();
        let detected = link::detect_profile(&profiles).unwrap_or_else(|error| {
            eprintln!("perfil: {error}");
            fm1_core::profile::generic()
        });
        println!("perfil: {} · {}", detected.id, detected.name);
        for error in errors {
            eprintln!("perfil: {error}");
        }
        if detected.id == "jiant-par12" {
            match link::read_show() {
                Ok(show) => {
                    println!("tempo: {} bpm", show.tempo);
                    println!("clock: {}", show.clock.name());
                    let labels: Vec<String> = show.items.iter().map(|item| item.label()).collect();
                    println!(
                        "set: {}",
                        if labels.is_empty() {
                            "(vacío)".to_string()
                        } else {
                            labels.join(" · ")
                        }
                    );
                    if show.chain.rows.is_empty() {
                        println!("cadena: (sin configurar)");
                    } else {
                        let rows: Vec<String> = show
                            .chain
                            .rows
                            .iter()
                            .map(|row| format!("{}({})", row.section, row.bars))
                            .collect();
                        println!("cadena: {}", rows.join(" "));
                    }
                    println!(
                        "pista: T{} · filtro {} · dj {} · size {} · mix {}",
                        show.selected + 1,
                        show.filter,
                        show.dj_filter,
                        show.size,
                        show.mix
                    );
                }
                Err(error) => eprintln!("lectura: {error}"),
            }
        } else {
            println!("params: {}", detected.params.len());
            for param in detected.params.iter().take(12) {
                println!(
                    "  {} {} {}..{}",
                    param.id, param.label, param.min, param.max
                );
            }
        }
        return Ok(());
    }

    if let Some(secs) = mix {
        let probe = link::Probe::start(link::default_watch(), false);
        let deadline = Instant::now() + Duration::from_secs(secs);
        let mut last: Option<link::Snapshot> = None;
        while Instant::now() < deadline {
            if let Some(snapshot) = probe.take_snapshot()
                && Some(snapshot.clone()) != last
            {
                println!(
                    "pista: T{} · valores: {:?}",
                    snapshot.selected + 1,
                    snapshot.values
                );
                last = Some(snapshot);
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 700.0])
            .with_min_inner_size([900.0, 560.0])
            .with_title("fm1lab"),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "fm1lab",
        options,
        Box::new(move |cc| {
            let mut app = PerfApp::new(cc, shot, demo, setlist, play_close, profile);
            if let Some(id) = firmware {
                app.open_library_at(Some(id));
            } else if library {
                app.open_library();
            }
            if profiles_open {
                app.open_profiles();
            }
            Ok(Box::new(app))
        }),
    )
}

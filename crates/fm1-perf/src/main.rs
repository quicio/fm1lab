//! `fm1-perf` — el panel de performance del M-VAVE FM-1.
//!
//! Sin webview a propósito (ADR-0002): la ventana dibuja con egui y cada tecla sale como byte MIDI en el
//! mismo frame. `--shot` guarda una captura y cierra: es la forma de verificar el dibujo sin mirar.

mod app;
mod link;
mod panels;
mod theme;
mod widgets;

use app::PerfApp;
use std::path::PathBuf;

const USAGE: &str = "\
uso: fm1-perf [--shot <archivo.png>] [--demo]

  --shot <archivo.png>   dibuja un frame, guarda la captura y sale (verificación sin ojos)
  --demo                 siembra el historial con envíos de ejemplo (para capturas y demos)
";

fn main() -> eframe::Result<()> {
    let mut shot: Option<PathBuf> = None;
    let mut demo = false;
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
            "--demo" => demo = true,
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
        Box::new(move |cc| Ok(Box::new(PerfApp::new(cc, shot, demo)))),
    )
}

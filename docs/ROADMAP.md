---
id: roadmap
status: vigente
version: 0.1.0
updated: 2026-11-11
---

# Roadmap

Cada milestone se cierra con un comando, no con una opinión. Los cambios concretos se proponen en
`openspec/`; acá está el orden y el criterio de salida.

## M0 — Dominio y CLI (cerrado)

Torre de tres capas, mapa NRPN generado, `fm1ctl` con `--dry`, puerta verde.

**Salida:** `scripts/check.sh` verde y `fm1ctl bpm 140 --dry` imprimiendo los cuatro mensajes.

## M1 — App de performance mínima (siguiente)

Ventana egui con: lista de setlist, disparo por teclado, indicador de conexión, y el estado del aparato
(canal, identidad). Se reusan `theme.rs`/`ui.rs` de `candil` como capa de presentación.

**Fuera de alcance:** dibujar perillas animadas, espectro, grabación.

**Salida:** `cargo run --profile fast -p fm1-perf -- --shot out.png --screen setlist` genera una captura
sin intervención (el patrón `--shot` de `candil`), y con el aparato conectado `section B` cambia de
sección desde la ventana.

## M2 — Persistencia y setlists reales

Guardar/cargar setlists en JSON atómico, con plantilla de fábrica y validación al cargar.

**Salida:** un setlist de 12 pasos se guarda, se reabre y dispara igual (`cargo test` + `fm1ctl` sobre el
archivo).

## M3 — Seguir el aparato (lectura)

Reconciliar la UI con el hardware: con MIDI OUT ALL encendido, el eco de NRPN actualiza los controles.
Evaluar el editor protocol (SysEx 1.0) si hace falta más que el eco.

**Salida:** girar una perilla en el aparato mueve el control correspondiente en la ventana.

## M4 — El estándar, publicado

Extraer de este repo la plantilla reutilizable: `AGENTS.md` + `docs/` + `scripts/check.sh` +
`.agents/skills/` + ADR de arranque, y un `cargo generate`-style seed para apps de escritorio livianas
en Rust (sin webview, puerta única, specs en openspec).

**Salida:** un repo nuevo creado desde la plantilla pasa su propia puerta en verde sin editar nada más
que el nombre.

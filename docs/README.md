---
id: docs-index
status: vigente
version: 0.1.0
updated: 2026-11-13
---

# Documentación

`fm1lab` — laboratorio de escritorio liviano para el M-VAVE FM-1: detecta el firmware, elige un
**perfil** y adapta el panel. Dominio en Rust sin infraestructura, adaptador MIDI aparte, UI egui.

## Orden de lectura

1. [`../AGENTS.md`](../AGENTS.md) — comandos y reglas duras (lo primero que lee un agente).
2. [`HANDOFF.md`](HANDOFF.md) — estado medido, invariantes, qué no hacer.
3. [`ROADMAP.md`](ROADMAP.md) — milestones y criterios de salida.
4. [`ARCHITECTURE.md`](ARCHITECTURE.md) — capas, módulos, protocolo.
5. [`NRPN.md`](NRPN.md) — el mapa de parámetros, copiado del firmware (fuente de la tabla generada).
6. [`../openspec/`](../openspec) — la fuente de verdad de los cambios.

## ADR

| id | decisión | estado |
|---|---|---|
| [0001](adr/0001-workspace-de-tres-crates.md) | workspace de tres crates con la regla de dependencia hacia adentro | aprobada |
| [0002](adr/0002-egui-032-sin-webview.md) | egui/eframe 0.32, sin webview (stack de `candil`, no Tauri) | aprobada |
| [0003](adr/0003-midir-como-dependencia.md) | `midir` como única dependencia de transporte | aprobada |
| [0004](adr/0004-mapa-nrpn-generado.md) | el mapa NRPN se genera desde el doc del firmware | aprobada |
| [0005](adr/0005-openspec-fuente-de-verdad.md) | `openspec/` gobierna los cambios; `docs/` describe el estado | aprobada |
| [0006](adr/0006-png-para-capturas.md) | `png` para la captura headless de `--shot` | aprobada |
| [0010](adr/0010-ruta-de-instalacion.md) | la ruta de instalación pasa por el oficial cuando cambia la familia | supersedida por 0011 |
| [0011](adr/0011-cambio-directo-entre-forks.md) | forks directos; el stock se instala con M-UPGRADE | aprobada |
| [0007](adr/0007-json-para-setlists.md) | JSON (`serde_json`) para persistir setlists | aprobada |
| [0008](adr/0008-perfiles-de-firmware.md) | el lab se organiza por perfiles de firmware, no por app | aprobada |
| [0009](adr/0009-instalador-ota-y-biblioteca.md) | instalador OTA en Rust (`fm1-flash`) + biblioteca con SHA-256 y backup | aprobada |

## Specs

Las specs de cambio viven en `openspec/specs/` (vigentes) y `openspec/changes/archive/` (histórico). Este
repo no mantiene un segundo árbol de specs: sería el error que `agent-handoff-package` advierte.

## Mapa del repo

```
crates/fm1-core/    dominio: nrpn (+ tabla generada), message, editor, profile, device, setlist, transport
crates/fm1-midi/    adaptador midir/CoreMIDI: puertos, editor, acceso CC/NRPN/editor, ClockMaster
crates/fm1-store/   adaptador de disco: setlist/panel/perfiles, catálogo y SHA-256, backup atómico
crates/fm1-flash/   instalador OTA/Felucca: paquete .fwsc, handshake, loader, verificación
crates/fm1ctl/      CLI (identidad, reloj, proyectos, cadena, setlist, firmware, flash, --dry)
crates/fm1-perf/    panel de performance (egui): app, link, library, theme, panels, widgets
design/             mock-panel.html — referencia visual del tema «Ceniza»
shots/              capturas de --shot (evidencia del dibujo)
scripts/            check.sh (la puerta), gen_nrpn.py (generador del mapa)
docs/               esta documentación + NRPN.md + ADR
openspec/           specs y cambios
.agents/skills/     clean-architecture · kent-beck-style · impeccable (vendorizadas)
.hermes/ .opencode/ skills y comandos que generó openspec init
```

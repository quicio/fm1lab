---
id: docs-index
status: vigente
version: 0.1.0
updated: 2026-11-11
---

# Documentación

`fm1lab` — app de escritorio liviana para operar un M-VAVE FM-1 en vivo. Dominio en Rust sin
infraestructura, adaptador MIDI aparte, UI egui.

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

## Specs

Las specs de cambio viven en `openspec/specs/` (vigentes) y `openspec/changes/archive/` (histórico). Este
repo no mantiene un segundo árbol de specs: sería el error que `agent-handoff-package` advierte.

## Mapa del repo

```
crates/fm1-core/    dominio: nrpn (+ tabla generada), message, device, setlist, transport
crates/fm1-midi/    adaptador midir/CoreMIDI
crates/fm1ctl/      CLI (identidad, mensajes, intenciones, setlists, --dry)
crates/fm1-perf/    panel de performance (egui): app, link, theme, panels, widgets
design/             mock-panel.html — referencia visual del tema «Ceniza»
shots/              capturas de --shot (evidencia del dibujo)
scripts/            check.sh (la puerta), gen_nrpn.py (generador del mapa)
docs/               esta documentación + NRPN.md + ADR
openspec/           specs y cambios
.agents/skills/     clean-architecture · kent-beck-style · impeccable (vendorizadas)
.hermes/ .opencode/ skills y comandos que generó openspec init
```

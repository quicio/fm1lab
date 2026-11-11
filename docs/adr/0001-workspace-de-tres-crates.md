---
id: adr-0001
title: Workspace de tres crates con la regla de dependencia hacia adentro
status: aprobada
date: 2026-11-11
supersedes: —
---

# 0001 — Workspace de tres crates

## Contexto

El proyecto hermano [`candil`](https://github.com/quicio/candil) resolvió el mismo problema (app de
escritorio en Rust, liviana, sin webview) en **un crate** y un `app.rs` que hoy tiene 10.473 líneas. Ese
archivo funciona, pero no se puede probar sin pantalla y absorbe cualquier responsabilidad nueva. Acá el
dominio tiene algo que `candil` no tenía: un protocolo externo (MIDI) con reglas propias que conviene
poder testear sin hardware.

## Decisión

Un workspace con tres crates:

- `fm1-core` — reglas: mapa de parámetros, codificación de mensajes, intenciones (`Device`), setlist. No
  conoce `midir`, ni archivos, ni egui.
- `fm1-midi` — adaptador: descubre puertos, abre la salida, lee en un hilo. Implementa los traits de
  `fm1-core::transport`.
- `fm1ctl` — entrega por CLI. La app de UI (`fm1-perf`) se suma igual, como otra entrega.

La regla: las dependencias de código apuntan hacia `fm1-core`. El dominio declara `MidiOut`/`MidiIn`; el
adaptador los cumple (inversión de dependencia).

## Consecuencias

- 22 tests corren sin aparato y sin pantalla, en milisegundos.
- El `--dry` de la CLI verifica bytes reales contra el mapa, sin abrir puertos.
- Cambiar de transporte (USB → TRS → grabador de prueba) es cambiar una línea de la entrega.
- Cuesta un `Cargo.toml` por crate y disciplina: si aparece `use midir` dentro de `fm1-core`, se rompió
  el acuerdo.

## Alternativas descartadas

- **Un crate con módulos.** Es lo que hizo un solo archivo de 10k líneas en `candil`: los tests terminan
  necesitando la UI.
- **Todo en la app.** Sin `fm1-core` no hay forma de correr un set por CLI ni de probar la codificación
  sin hardware.

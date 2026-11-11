---
id: adr-0002
title: egui/eframe 0.32 sin webview (se reusa el stack de candil, no Tauri)
status: aprobada
date: 2026-11-11
supersedes: —
---

# 0002 — egui 0.32, sin webview

## Contexto

La app es un panel de performance: un click tiene que salir como byte MIDI en pocos milisegundos, la
ventana tiene que abrir al instante y el binario tiene que ser chico. Las opciones eran Tauri (webview del
sistema + IPC + timers del frontend) o el stack que ya está probado en `candil`: `eframe 0.32` con
`default-features = false` y las features `default_fonts, glow, wayland, x11`.

## Decisión

egui/eframe 0.32, pinneado. Se copian de `candil` la capa de presentación (`ui.rs`, `theme.rs`) y no su
forma de app.

## Consecuencias

- Binario de pocos MB (en `candil`, `dist` = 7,1 MiB), arranque instantáneo, sin motor HTML ni CSS.
- El camino del click al byte no cruza un puente web: `Ui` → dominio → adaptador.
- Se hereda el pin: **egui 0.37 es una reescritura** (`App::ui`, sin `TopBottomPanel`). Subir de versión
  es un ADR nuevo y un trabajo de migración, no un bump de `Cargo.toml`.
- No hay `impeccable detect` ni modo `live`: su motor de reglas es HTML/DOM y su propia
  `reference/routing.md` dice que no aplica a apps nativas. De esa skill se usa el vocabulario y la
  rúbrica, aplicadas a mano.
- egui no trae perilla: hay que dibujarla con `Painter` (~30 líneas). Se asume.

## Alternativas descartadas

- **Tauri 2.** Aporta webview, IPC y ecosistema web que acá no hacen falta, y alarga el camino crítico.
- **Electron.** Descartado por peso y arranque.
- **Slint / iced.** Viables, pero implican reescribir la capa de presentación que `candil` ya tiene
  resuelta y probada.

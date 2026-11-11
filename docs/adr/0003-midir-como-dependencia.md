---
id: adr-0003
title: midir como única dependencia de transporte
status: aprobada
date: 2026-11-11
supersedes: —
---

# 0003 — `midir` para MIDI

## Contexto

La regla del repo es que ninguna dependencia entra sin justificar peso, licencia y mantenimiento. Para
hablar MIDI en macOS hay tres caminos: `midir` (CoreMIDI en macOS, WinMM en Windows, ALSA en Linux),
`coremidi` a pelo (macOS only), o llamar al framework por FFI propio.

## Decisión

`midir 0.11` (MIT), y sólo en `fm1-midi`. El dominio no lo ve.

## Consecuencias

- Un crate de dependencia directa; arrastra `coremidi`, `core-foundation`, `libc`, `objc2` en macOS.
  En `candil` esa cadena ya existe para `eframe`, así que el costo marginal es bajo.
- Multiplataforma gratis: el mismo adaptador compila en Linux (ALSA) y Windows (WinMM) sin cambios.
- Sin hot-plug: `midir` no notifica cuándo aparece o desaparece un aparato. Hay que sondear `ports()`
  (cada instalación de firmware hace desaparecer el puerto y volver con otro nombre).
- Nombres de puerto cambiantes según firmware (`FM-1`, `JIANT`, `JIANT FM`, `Felucca`): se busca por
  subcadena, nunca por nombre exacto.

## Alternativas descartadas

- **`coremidi` directo.** Sólo macOS; obliga a un adaptador por sistema operativo.
- **`midir` en todas las capas.** Filtraría infraestructura al dominio (ver ADR-0001).

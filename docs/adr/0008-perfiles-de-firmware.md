---
id: adr-0008
title: fm1lab como laboratorio de perfiles de firmware
status: aprobada
date: 2026-11-13
supersedes: —
---

# 0008 — Perfiles de firmware, no una app por firmware

## Contexto

`fm1lab` nació atado a JIANT: el editor protocol, las secciones/proyectos, el reloj USB y la tabla de
239 parámetros están hardcodeados en `fm1-core`, `fm1-midi` y `fm1-perf`. Eso sirve para JIANT, pero el
objetivo real es **probar firmwares** (JIANT, X0X, TRIBE, SLOOP, stock, dock): cada uno expone un MIDI
distinto, un mapa de parámetros distinto y una forma distinta de arrancar el transporte.

Mantener una app por firmware multiplica el mantenimiento y congela el lab. Una app que ignore el
firmware y sólo hable CC estándar pierde lo que hace útil a JIANT (editor, secciones, lectura real).

## Decisión

`fm1lab` se organiza alrededor de **perfiles**: una descripción declarativa (JSON) de lo que un
firmware expone, más adaptadores Rust sólo para los protocolos que no son MIDI estándar.

Un perfil declara:

- **Identidad**: cadenas de identidad/versión que lo reconocen (handshake OTA e INFO del editor).
- **Parámetros**: id, nombre, scope (global/pista), rango, default y **acceso**:
  - `cc { channel, cc }`
  - `nrpn { channel, msb, lsb }`
  - `editor { scope, id }` (SysEx propio de JIANT)
  - `none`/sólo lectura.
- **Transporte**: `clock_master` (la app manda Start/clock; opcionalmente setea `CLOCK` por editor),
  `midi` (Start/Stop/clock genéricos) o `none`.
- **Escenas**: `program_change` (JIANT: secciones/proyectos), `patterns`, `clips`, `none`.
- **Capacidades**: lectura por polling, learn, monitor, backup.

El lab:

1. **Detecta** el firmware (identidad OTA + INFO) y elige un perfil; `--profile <id>` lo fuerza.
2. **Cae a `generic`** (mapa CC estándar del FM-1, 6 canales de pista) cuando no reconoce el firmware.
3. **Muestra sólo lo que el perfil soporta**: franjas, transporte, setlist/escenas.
4. **Deja asignar** cualquier parámetro del catálogo del perfil a las franjas, y lo guarda por perfil.
5. **Monitorea** el MIDI de entrada y, en fase 2, **aprende** un CC/NRPN que el usuario mueva para
   agregarlo al perfil sin recompilar.

El flasheo **no** entraba al lab: quedaba en `m-vave` (`jiant/fm1_install.py`, `firmware/*.fwsc`,
`fm1-dock/tools`). El lab sólo observaba y mandaba MIDI. (Esta parte la reemplaza ADR-0009: hoy la
biblioteca instala con `fm1-flash`, checksum y backup.)

## Consecuencias

- JIANT pasa a ser un **perfil** (`jiant-par12`) con su editor, secciones y reloj; nada de lo que ya
  funciona se tira.
- `generic` funciona con stock/X0X/TRIBE/SLOOP en lo que sea CC estándar, sin conocer el firmware.
- Agregar un firmware con MIDI estándar es **editar un JSON**; uno con SysEx propio suma un adaptador
  chico en `fm1-midi`.
- El estado y los setlists guardan el `profile_id`; si cambiás de firmware, se cae al perfil genérico
  y las asignaciones que no existan se avisan, no se pisan.
- El costo es una capa de indirección y un formato de perfil que hay que mantener; se paga con no
  reescribir la app por firmware.

## Alternativas descartadas

- **Una app por firmware.** Duplica UI, tests y deuda; el lab deja de ser lab.
- **Sólo CC estándar.** Pierde el editor de JIANT (lectura real, secciones, `CLOCK`).
- **Un DAW genérico.** Otro producto, otro peso; acá se quiere un panel chico y una puerta de
  verificación sin ojos.

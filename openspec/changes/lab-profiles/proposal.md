## Why

`fm1lab` está atada a JIANT: el editor protocol, las secciones/proyectos, `CLOCK=USB` y el mapa de 239
parámetros están en el código. El objetivo del proyecto es **probar firmwares** (JIANT, X0X, TRIBE,
SLOOP, stock, dock), y cada uno expone un MIDI distinto. Sin una capa de perfiles, cada firmware nuevo
implica reescribir la app; con perfiles, es un JSON (o un adaptador chico para un SysEx propio).

## What Changes

- **Perfiles declarativos** (JSON): identidad, parámetros con su **acceso** (`cc`, `nrpn`, `editor`),
  transporte y escenas/capacidades. Built-in `generic` y `jiant-par12`; el usuario puede sumar
  perfiles en `~/.config/fm1lab/profiles/`.
- **Detección**: identidad OTA + INFO del editor → perfil; `--profile <id>` lo fuerza; si no reconoce,
  `generic`.
- **Acceso genérico**: `read`/`write`/`watch` de un parámetro según su acceso; el editor de JIANT queda
  como un adaptador, no como el único camino.
- **Franjas asignables**: el picker (`a`) lista los parámetros del perfil; la asignación se guarda por
  perfil en `panel.json`.
- **UI por capacidades**: el transporte y el setlist/secciones aparecen sólo si el perfil los declara.
- **JIANT sigue funcionando** como perfil: editor, secciones, reloj USB, lectura real.
- **Fase 2 (mismo cambio, al final): monitor + learn** de CC/NRPN para agregar parámetros a un perfil
  sin recompilar.

## Capabilities

### New Capabilities

- `device-profiles`: detectar/elegir un firmware y describir parámetros, accesos, transporte y escenas.
- `assignable-strips`: elegir qué parámetros del perfil controla cada franja y persistirlo.
- `midi-monitor`: ver el MIDI de entrada y aprender un CC/NRPN para el perfil (fase 2).

### Modified Capabilities

- `device-follow`: el sondeo sigue los parámetros asignados del perfil, no cuatro ids fijos.
- `device-write`: escribe por el acceso del perfil (`editor`, `cc` o `nrpn`) y confirma cuando el
  protocolo lo permite.

## Impact

- `fm1-core`: modelo `Profile`/`Param`/`ParamAccess`, perfiles built-in, `Scope` serializable.
- `fm1-midi`: `read`/`write`/`watch` por acceso (editor/CC/NRPN); transporte según perfil.
- `fm1-store`: perfiles de usuario y `panel.json` por perfil; setlist con `profile_id`.
- `fm1-perf`: detección/perfil, picker, franjas desde el perfil, UI por capacidades.
- `fm1ctl`: `profile`, `params --profile`, `monitor` (fase 2).
- `PRODUCT.md`, `AGENTS.md`, `HANDOFF.md`, `ARCHITECTURE.md`, ADR-0008.

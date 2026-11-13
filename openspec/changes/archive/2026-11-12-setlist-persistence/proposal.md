## Why

El set hoy sale del aparato o de la demo en código: no se puede editar, guardar ni reusar entre
sesiones. M2 pide setlists reales en disco, con formato estable y sin riesgo de perder el show por un
archivo a medio escribir.

## What Changes

- **Crate nuevo `fm1-store`** (adaptador, sin MIDI): guarda y carga un setlist en JSON con versión,
  escritura atómica (temporal + `rename`), validación de rangos al cargar, copia `.bak` si el archivo
  está corrupto y plantilla de fábrica si no existe.
- La app **carga el archivo al arrancar**, `s` guarda el set actual y `l` lo vuelve a cargar; `r`
  cambia al set del aparato. `--setlist <archivo>` elige otro archivo.
- `fm1ctl setlist <archivo> [--dry]` valida y muestra el set (con `--dry`, los bytes de cada paso);
  `fm1ctl setlist init <archivo>` escribe la plantilla.
- **ADR-0007**: `serde_json` pasa de dependencia de test a runtime (ya estaba en el lock) para el
  formato del setlist.

## Capabilities

### New Capabilities

- `setlist-persistence`: formato de archivo, guardado atómico, carga validada, plantilla y recuperación
  de archivos corruptos.

### Modified Capabilities

<!-- No cambia el comportamiento de las specs vigentes: device-read/device-write/performance-transport
     siguen igual; el setlist persistido es una fuente más para la app. -->

## Impact

- `crates/fm1-store/` (nuevo): store JSON, ruta por defecto y `FM1LAB_SETLIST`.
- `fm1-perf`: carga/guardado, teclas `s`/`l`, `--setlist`.
- `fm1ctl`: `setlist` e `init --dry`.
- `docs/adr/0007-json-para-setlists.md`, `HANDOFF.md`, `ARCHITECTURE.md`, `ROADMAP.md`.
- `scripts/check.sh`: humo de ida y vuelta con un archivo temporal.

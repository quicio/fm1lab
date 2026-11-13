## Why

Probar firmwares implica cambiarlos seguido. Hasta ahora eso se hacía con un script Python aparte
(`m-vave/x0x/fm1_install.py`), sin catálogo, sin checksum visible, sin backup y sin saber qué versión
está instalada. El lab ya detecta el firmware y tiene perfiles; el paso que falta es **elegir e
instalar** el firmware desde el mismo producto, con una vuelta atrás segura.

El protocolo OTA/Felucca está probado por el instalador de `m-vave`, pero es la operación más riesgosa:
escribe la flash y no hay ACK. La decisión (ADR-0009) es portarlo a Rust y envolverlo en un flujo con
verificación, respaldo y confirmación explícita.

## What Changes

- **Catálogo de firmwares**: `firmwares.json` built-in (embebido) + `~/.config/fm1lab/firmwares.json`;
  búsqueda por nombre, familia, versión, etiqueta o nota. Cada entrada declara identidad OTA, archivo,
  SHA-256, etiquetas, notas y perfil sugerido.
- **Estado del paquete**: presente/ausente, SHA-256 correcto o no, y qué firmware reporta el aparato.
- **Instalador Rust** (`fm1-flash`): parseo del `.fwsc`, handshake, loader, respuesta de pedidos y
  verificación de identidad al reiniciar. `fm1ctl flash` y la ventana usan el mismo camino.
- **Backup antes de escribir**: setlist, panel, catálogo, perfiles y todos los `.fwsc` a
  `~/.config/fm1lab/backups/<fecha>/`; si falla, no se escribe.
- **Confirmación y pausa**: la ventana exige marcar la confirmación; la CLI exige `--yes`. Antes del
  flash se para el transporte y se pausan el sondeo y el monitor.
- **CLI**: `fm1ctl firmware [query]` y `fm1ctl firmware --install <id> [--dry|--yes] [--force]`.
- **UI**: tecla `f`, ventana buscable con ficha, estado, confirmación, progreso y log.

## Capabilities

### New Capabilities

- `firmware-library`: catálogo buscable, verificación, backup e instalación OTA con confirmación.

### Modified Capabilities

- `device-profiles`: el perfil sugerido de cada firmware queda en el catálogo; al terminar el flash la
  app re-detecta el perfil y lo cambia en caliente.

## Impact

- `fm1-store`: `catalog.rs` (catálogo, rutas, estado, backup) y `sha256.rs` (hash propio con tests).
- `fm1-flash`: crate nuevo con el port del protocolo OTA/Felucca.
- `fm1-perf`: `library.rs` y la tecla `f`; `Probe`/`Monitor` ganan pausa.
- `fm1ctl`: `firmware` y backup automático en `flash`.
- `firmwares.json` en la raíz; ADR-0009; `PRODUCT.md`, `AGENTS.md`, `HANDOFF.md`,
  `ARCHITECTURE.md`, `ROADMAP.md`.

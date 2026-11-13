## 1. Store y decisión de dependencia

- [x] 1.1 ADR-0007: `serde_json` de test a runtime para el formato del setlist (ya estaba en el lock).
- [x] 1.2 Crate `fm1-store` en el workspace, con `fm1-core`, `serde` y `serde_json`.

## 2. Formato, ruta y escritura atómica

- [x] 2.1 `load` / `save` con wrapper versionado, `#[serde(flatten)]` y `Setlist::validate()`.
- [x] 2.2 Guardado atómico (temporal en el mismo directorio + `sync_all` + `rename`), limpieza del
      temporal si falla.
- [x] 2.3 Corrupto: mover a `.bak` y devolver error; ausente: `NotFound`.
- [x] 2.4 Plantilla A–D, `init`, ruta por defecto y `FM1LAB_SETLIST`.
- [x] 2.5 Tests: ida y vuelta de 12 pasos, versión desconocida, paso inválido, corrupto con `.bak`,
      temporal que no queda.

## 3. CLI

- [x] 3.1 `fm1ctl setlist <archivo> [--dry]`: valida, lista pasos y con `--dry` imprime los bytes.
- [x] 3.2 `fm1ctl setlist init <archivo>`: escribe la plantilla.
- [x] 3.3 Humo en `check.sh` con un archivo temporal (init → `--dry` → limpiar).

## 4. App

- [x] 4.1 Cargar el archivo al arrancar; `Origin::{File, Device}` y el set del aparato no pisa el
      archivo.
- [x] 4.2 Teclas `s` (guardar) y `l` (cargar), con estado en el pie; `r` sigue trayendo el set del
      aparato.
- [x] 4.3 `--setlist <archivo>` y el pie actualizado.

## 5. Verificación y docs

- [x] 5.1 `cargo test`, `scripts/check.sh` verde.
- [x] 5.2 Ida y vuelta automatizada: 12 pasos en `cargo test`, `fm1ctl setlist` sobre el archivo y la
      app cargando con `--setlist` (verificado con captura).
- [x] 5.3 Prueba manual en la ventana: `s` guarda y `l` vuelve a cargar el set.
- [x] 5.4 `--shot` regenerado; `HANDOFF.md`, `ARCHITECTURE.md`, `ROADMAP.md` y `README` al día.
- [x] 5.5 `openspec validate --specs --strict`.

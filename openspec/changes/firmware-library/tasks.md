## 1. Instalador

- [x] 1.1 Crate `fm1-flash`: `Package` (identidad, image lógico, loader), `Installer`, `Event`, errores.
- [x] 1.2 Handshake, check, loader, write, verify; timeouts del instalador probado de `m-vave`.
- [x] 1.3 Tests de `pack7`/`unpack7`, respuesta, request y parseo de identidad.
- [ ] 1.4 Primer flash real (X0X 1.2.2) con verificación de identidad y vuelta atrás.

## 2. Catálogo y checksum

- [x] 2.1 `firmwares.json` built-in con X0X, JIANT par12/gen2, dock y stock; SHA-256 calculados.
- [x] 2.2 `fm1-store::catalog`: carga embebida + usuario, merge por `id`, validación, búsqueda.
- [x] 2.3 `sha256.rs` propio con vectores conocidos; `state_of` listo/falta/no coincide.
- [x] 2.4 `backup_lab`: config, perfiles, catálogo y `.fwsc` a `backups/<fecha>/`.
- [x] 2.5 Tests de catálogo, usuario que pisa built-in, sha, fecha y backup.

## 3. CLI

- [x] 3.1 `fm1ctl firmware [query]` con identidad del aparato, estado y `*` en el instalado.
- [x] 3.2 `fm1ctl firmware --install <id> [--dry|--yes] [--force]`.
- [x] 3.3 `fm1ctl flash` respalda antes de escribir; `--dry` sólo parsea.
- [x] 3.4 Verificación en seco de los cinco paquetes y del stock sin `--force`.

## 4. Ventana

- [x] 4.1 `library.rs`: búsqueda, lista, ficha, estado del paquete e identidad instalada.
- [x] 4.2 Confirmación + `--force`, backup y flash en un hilo con progreso y log.
- [x] 4.3 `f` abre/cierra; `Probe` y `Monitor` se pausan; el transporte se para antes.
- [x] 4.4 `--library` y captura `shots/firmware.png`.

## 5. Documentación y puerta

- [x] 5.1 ADR-0009 y actualización de ADR-0008.
- [x] 5.2 `PRODUCT.md`, `AGENTS.md`, `HANDOFF.md`, `ARCHITECTURE.md`, `ROADMAP.md`.
- [x] 5.3 `scripts/check.sh` verde y docs con métricas nuevas.
- [ ] 5.4 Prueba de la ventana con el aparato conectado (marca el instalado y muestra los CC).

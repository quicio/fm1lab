## 1. Dominio: editor protocol en `fm1-core`

- [x] 1.1 `editor.rs`: tramas `F0 7D 46 4C cmd args F7`, offset binary (`valor + 8192`), constantes
      de comandos (`INFO`, `GET`, `SET`, `PROJECT`, `SONG`) y `Clock { Int, Usb, Trs }` con su id 2.
- [x] 1.2 Parseo de respuestas con las capturas reales: `PROJECT` (slot + usado), `SONG` (filas),
      `GET` (global), `INFO` (versión). Tests unitarios por cada una y por el offset binary.
- [x] 1.3 `Device::start/stop/clock/set_clock` (realtime 0xFA/0xFC/0xF8 y `SET` del reloj) con tests
      de los bytes emitidos.
- [x] 1.4 Las consultas con espera viven en el adaptador (`Port::editor`); el dominio sólo arma y
      parsea, sin I/O.

## 2. Adaptador y lectura

- [x] 2.1 `fm1-midi`: `Port::editor(request, wait)` (mandar SysEx y esperar la respuesta del `cmd`).
- [x] 2.2 `fm1-perf/link.rs`: consultar los 4 slots, la cadena y el BPM; armar `Show` con las
      secciones usadas y descartar las filas vacías.
- [x] 2.3 `Probe` publica el `Show` en un `Arc<Mutex<...>>` sin bloquear el dibujo; pedido al
      arrancar y con `r`.

## 3. App: transporte y setlist real

- [x] 3.1 `link::Transport`: abrir puerto, leer `CLOCK`, poner `USB`, mandar `Start`, hilo de clock a
      `bpm × 24` con deadlines, `Drop` que para y restaura.
- [x] 3.2 `app.rs`: estado de transporte real, tecla `espacio`, `fire` que arranca si está parado y
      que para/carga/rearranca para un proyecto.
- [x] 3.3 Al armar el set, tomar el `Show` y el BPM del aparato; `r` vuelve a leer; sin aparato,
      conservar el set anterior y avisar.
- [x] 3.4 `panels/chrome.rs`: indicador PLAY/STOP y la tecla en el pie.

## 4. CLI

- [x] 4.1 `fm1ctl clock [int|usb|trs]` (leer o escribir, `--dry` imprime la trama).
- [x] 4.2 `fm1ctl projects` y `fm1ctl song` imprimen lo mismo que lee la app.

## 5. Verificación

- [x] 5.1 `cargo test` con los tests nuevos del editor y del dominio.
- [x] 5.2 `scripts/check.sh` verde (fmt, clippy, tests, mapa al día, humo).
- [x] 5.3 Prueba en vivo: play/stop con audio, sección durante el clock, reloj restaurado a INT y
      set leído del proyecto 1 (A/B/A/C, sin la D vacía).
- [x] 5.4 `--shot` regenerado y revisado.

## 6. Documentación

- [x] 6.1 `HANDOFF.md` (teclas, estado, rescate `fm1ctl clock int`), `ARCHITECTURE.md` (editor y
      clock) y `ROADMAP.md` (M3 en curso).
- [x] 6.2 `openspec validate --specs --strict` y cambio listo para archivar.

## 7. Escritura por el editor (NRPN no implementado en par12)

- [x] 7.1 `Device::set_bpm` / `set_global_editor` / `set_selected_track` con sus frames y tests.
- [x] 7.2 La app escribe tempo y franjas por el editor; el filtro va sobre la pista seleccionada y se
      rotula con la que dice `TRACK`.
- [x] 7.3 Verificación en vivo de ida y vuelta: BPM, FILTER, SIZE y DJ FILT con `GET` después de `SET`.

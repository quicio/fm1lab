## Why

Disparar un paso del set no hace sonar nada: la JIANT ignora el `Start` de MIDI cuando
`CLOCK = INT` (el default) y la app no tiene play/stop. Además la setlist de ejemplo es fija
(`proyecto 3`, `proyecto 2`, …) y apunta a proyectos vacíos, así que la app saca al aparato de la
música que sí existe y no tiene forma de saberlo porque el aparato no manda ACK.

## What Changes

- La app pasa a ser **dueña del reloj** mientras reproduce: lee el `CLOCK` actual del aparato, lo pone
  en `USB`, manda `Start` y 24 pulsos de clock por negra al tempo de la app; al parar manda `Stop` y
  **restaura** el `CLOCK` que tenía. Si el envío falla, restaura igual y avisa.
- Tecla de transporte (`espacio`) y estado visible en la cabecera. `1–6` arranca la reproducción si
  estaba parada; si ya estaba sonando, sólo salta de sección (al próximo compás, como el firmware).
- La setlist deja de ser fija: al arrancar y al pedir `r`, la app **lee** por el editor protocol las
  cuatro secciones del proyecto cargado, la cadena de song y el BPM real, y arma el set con lo que
  tiene música. Las secciones vacías no aparecen y el tempo de la app arranca con el del aparato.
- Los parámetros que toca el panel (tempo y las cuatro franjas) pasan a escribirse por el **editor
  protocol**: el firmware 0.9.9 no implementa NRPN, así que el camino anterior no movía nada.
- `fm1ctl` gana `clock`, `projects` y `song` para verificar todo esto sin abrir la ventana.

## Capabilities

### New Capabilities

- `performance-transport`: play/stop desde la app como reloj maestro, con restauración del `CLOCK`
  del aparato y aviso de fallo.
- `device-read`: lectura del editor protocol (secciones usadas, cadena de song, BPM y `CLOCK`) para
  que el panel muestre y dispare lo que el aparato tiene de verdad.
- `device-write`: escritura del tempo y de las franjas por el editor protocol, con el id y el valor
  crudo del mapa, porque el firmware no implementa NRPN.

### Modified Capabilities

<!-- No hay specs previas en openspec/specs/ todavía. -->

## Impact

- `fm1-core`: módulo `editor` (tramas SysEx, offset binary, parseo con capturas reales), `Clock`,
  `Device::start/stop/clock/set_clock` y lectura con `MidiIn`.
- `fm1-midi`: `Port::editor` (pedido/respuesta con timeout) y apertura persistente para el reloj.
- `fm1-perf`: `link.rs` (lectura en hilo + `Transport` con hilo de clock), `app.rs` (estado real de
  transporte, setlist leída, `espacio`), `panels/chrome.rs` (estado).
- `fm1ctl`: `clock`, `projects`, `song`.
- Docs: `HANDOFF.md`, `ARCHITECTURE.md`, `ROADMAP.md`. Sin dependencias nuevas.

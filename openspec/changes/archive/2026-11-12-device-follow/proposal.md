## Why

M3 pide que la ventana siga al aparato. El camino documentado (eco de NRPN con MIDI OUT ALL) no
existe en el firmware 0.9.9: su salida MIDI sólo manda notas y clock, y no hay handler de NRPN. La
única lectura real es el editor protocol, que ya se usa para el setlist y el reloj.

## What Changes

- El hilo de sondeo **consulta cada ~600 ms** la pista seleccionada y los cuatro valores de las
  franjas (`FILTER`, `DJ FILT`, `SIZE`, `MIX`) por el editor protocol, y publica un snapshot `Mix`.
- La app aplica ese snapshot a las franjas (valor y rótulo `cabezal tN`), sin tocar el setlist ni el
  tempo. Un cambio hecho en el aparato se ve en la ventana al siguiente sondeo.
- `fm1ctl mix` imprime los mismos valores, para verificar el seguimiento sin abrir la ventana.

## Capabilities

### New Capabilities

- `device-follow`: la ventana refleja los valores de las franjas que tiene el aparato, leídos por el
  editor protocol, sin bloquear el dibujo.

### Modified Capabilities

<!-- device-read sigue igual: el snapshot inicial del set ya traía las franjas; esto agrega el
     seguimiento continuo. -->

## Impact

- `fm1-perf/src/link.rs`: `Mix`, `read_mix` y sondeo en el hilo del `Probe`.
- `fm1-perf/src/app.rs`: aplicar el snapshot a las franjas, con guarda para no pelear con un `↑↓`.
- `fm1ctl`: `mix`.
- `docs/HANDOFF.md`, `ARCHITECTURE.md`, `ROADMAP.md` (M3).

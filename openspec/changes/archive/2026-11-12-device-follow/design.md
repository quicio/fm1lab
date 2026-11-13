## Context

Ver `proposal.md`. La salida MIDI del firmware (`seq.c` `mo_send`) sólo manda notas y clock; no hay
eco de NRPN ni handler de NRPN de entrada. El editor protocol ya resuelve `GET` y `TRACK`, y el
`Probe` ya corre en un hilo aparte. El panel tiene cuatro valores que le importan: `FILTER` (scope 0,
id 5, pista seleccionada), `DJ FILT` (global 17), `SIZE` (8) y `MIX` (7).

## Goals / Non-Goals

**Goals:**

- Reflejar en ≤1 s los cuatro valores y la pista seleccionada.
- Sin bloquear el dibujo y sin tocar el set ni el tempo.
- Una verificación headless (`fm1ctl mix`).

**Non-Goals:**

- Reconciliar todos los parámetros del aparato (239): sólo las cuatro franjas y la pista.
- Seguir el transporte (el firmware no lo expone en par12 sin el contrato 1.0).
- Parsear el eco de notas/clock.

## Decisions

### Polling del editor, no eco de NRPN

- Medido en el código y en la unidad: no hay eco de NRPN. La única lectura es `GET`.
- Cinco consultas por vuelta (`TRACK` + cuatro `GET`) cada ~600 ms. Cada una tarda milisegundos; el
  editor contesta en su loop principal. Es barato y no depende de `MIDI OUT ALL`.

### En el hilo del `Probe`, no en el dibujo

- El `Probe` ya sondea puertos y lee a pedido. Se le agrega un `Mix` que publica cada vuelta; la UI
  lo consume con `take_mix` y nunca espera.
- El ciclo baja a 500 ms con descubrimiento cada 3 vueltas (1,5 s), para no enumerar puertos de más.

### Snapshot `Mix` separado del `Show`

- `Show` lee el set, la cadena y el tempo (11 consultas); no sirve para poll. `Mix` es sólo lo que
  cambia con una perilla.
- La app aplica `Mix` a las franjas y no toca `setlist` ni `tempo`.

### Guarda contra el sondeo que llega tarde

- `touched` ya marca la franja movida. Si el `Mix` llega dentro de los 400 ms de un `↑↓`, esa franja
  no se pisa; el próximo sondeo confirma el valor del aparato.

### Verificación

- `fm1ctl mix` usa el mismo `read_mix` que el `Probe`; con un `SET` por el editor (o girando la
  perilla) se comprueba la lectura. La prueba de la UI es girar una perilla con la ventana abierta.

## Risks / Trade-offs

- **Tráfico cada 600 ms** → cinco tramas cortas; el editor ya atiende al reloj y a las escrituras.
  Si el aparato está ocupado, una consulta puede tardar y el snapshot se saltea.
- **La pista seleccionada cambia** → el `TRACK` y el `GET` de filtro no son atómicos; si el usuario
  cambia de pista justo entre los dos, un sondeo puede mezclar. El siguiente lo corrige.
- **Un `↑↓` y un sondeo viejo** → la guarda de 400 ms evita el parpadeo; el valor del aparato gana
  después.
- **Aparato ausente** → se conserva el último snapshot y no se inunda la UI de errores.

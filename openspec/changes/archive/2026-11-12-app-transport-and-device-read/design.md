## Context

Ver `proposal.md`. Lo medido en la unidad (JIANT 0.9.9-par12, `FM-1_909`) que condiciona el diseño:

- El editor protocol usa tramas `F0 7D 46 4C cmd args F7` y **offset binary**: el valor viaja como
  `valor + 8192`, al revés que el NRPN (que escala sobre el rango). Mezclarlos hace que un `SET` de
  `CLOCK` escriba `INT` cuando se quiere `USB` (pasó en la prueba).
- `CLOCK` es el global 2 (`INT`, `USB`, `TRS`). No está en el mapa NRPN: se toca con `SET`, no con
  NRPN.
- Con `CLOCK = INT`, el `Start`/`Stop`/clock que llegan por USB **se ignoran**: el firmware sólo los
  atiende si `G_CLOCK == 1` (USB). Con `CLOCK = USB` y clock entrante, el transporte anda.
- `PROJECT` (cmd 9, op 2) dice si una sección tiene música; `SONG` (cmd 33, op 0) da las filas de la
  cadena; `GET` (cmd 2) da el valor de un global.
- La cadena del song puede tener filas que apuntan a secciones vacías (la unidad tenía A/B/C/D y D
  vacía): hay que filtrarlas antes de armar el set.

## Goals / Non-Goals

**Goals:**

- Play/stop real desde la app, con el aparato como esclavo del reloj USB.
- Set y tempo derivados de lo que el aparato tiene cargado.
- Lectura sin bloquear el hilo de dibujo.
- Dejar el aparato como estaba (reloj y transporte) al parar o salir.

**Non-Goals:**

- Leer o cambiar los proyectos que no están cargados (implicaría cambiar de song y tocar flash).
- Persistir setlists en disco (M2).
- Seguir el reloj del aparato o reconciliar el eco de NRPN (M3, otra parte).
- Sincronizar por TRS.

## Decisions

### El protocolo vive en `fm1-core`, la I/O en `fm1-midi`

- `fm1-core/src/editor.rs` arma y parsea tramas (puro bytes, testeable sin aparato). `Clock` es un
  enum del dominio.
- `fm1-midi` agrega `Port::editor(request, wait)`: manda el SysEx y espera la respuesta con el mismo
  `cmd`, con timeout. Es el único que conoce CoreMIDI, como manda la regla de dependencia.
- `fm1-perf/link.rs` orquesta las consultas (qué preguntar y cómo armar el set) porque el modelo del
  show es de la app, no del adaptador.
- Alternativa descartada: poner el parseo en `fm1-midi`. Duplicaría el contrato del firmware fuera
  del dominio y no se podría testear sin puerto.

### Escritura por el editor, no por NRPN

- Medido en par12: `docs/NRPN.md` documenta NRPN, pero el firmware **no tiene handler**: CC99/98 sólo
  cancelan el RPN y CC6/38 escriben el bend range. Mover el BPM o las franjas por NRPN no hacía nada.
- La app escribe tempo y franjas con `SET scope/id` del editor, usando los ids y rangos del mapa y el
  valor crudo (offset binary). `GET` devuelve lo mismo: el test es ida y vuelta contra el aparato.
- La franja de filtro va sobre la pista seleccionada (scope 0); `TRACK` dice cuál es y la UI la rotula.
- Alternativa descartada: seguir con NRPN "porque el mapa lo dice". El mapa es el contrato del
  firmware, no su implementación; la prueba en hardware manda.

### Reloj maestro con hilo propio

- `link::Transport` abre un `Port` persistente, lee el `CLOCK` vigente, lo pone en `USB`, manda
  `Start` y lanza un hilo que emite `0xF8` cada `60s / (bpm × 24)`.
- El hilo usa deadlines de `Instant` y hace *spin* en el último milisegundo; a 140 BPM son 17,9 ms
  por pulso, así que el error de `sleep` no importa. El tempo se comparte con un `AtomicU32`
  (centésimas de BPM) para que `±` lo cambie sin reiniciar el hilo.
- Al parar (o al fallar un envío, o en `Drop`), el mismo hilo manda `Stop`, deja el BPM en el tempo
  usado (el firmware estima el BPM del clock y lo guarda: sin esto la próxima lectura arrastra el
  error) y restaura el `CLOCK` original. Cada `SET` espera su respuesta antes del siguiente: dos
  tramas seguidas con el puerto cerrándose hacen que el aparato se quede con la primera. El `Drop` es
  la red para cerrar la ventana con el transporte andando.
- Alternativa descartada: agendar el clock desde `update` de egui. Los repintados no son un reloj y
  el diseño ya lo prohíbe.

### Un `Port` para el clock y los envíos por comando como ahora

- El hilo del clock mantiene su conexión; cada disparo sigue abriendo y cerrando un `Port` (patrón
  actual, ya verificado: un note-on con abrir→mandar→cerrar suena). CoreMIDI permite dos clientes
  sobre el mismo aparato.
- Alternativa descartada: compartir un único `Port` con un `Mutex`. Serializa el hilo de dibujo con
  el clock y hace que un bloqueo del envío pare el reloj.

### La lectura la hace el hilo de sondeo

- `Probe` ya corre fuera del dibujo para descubrir el puerto. Se extiende para publicar un snapshot
  `Show { secciones, bpm }` cuando hay un pedido (`flags`: arranque o tecla `r`).
- La UI sólo lee el snapshot con `try_lock`; nunca espera al aparato.
- Timeout por consulta: 350 ms; si algo no contesta, se conserva el set anterior y se muestra el
  error.

### Set a partir de la cadena, filtrado por secciones usadas

- Se consultan los 4 slots del proyecto actual y la cadena. El set es la cadena sin las filas cuya
  sección está vacía; si no hay cadena, las secciones usadas en orden A–D.
- El BPM se lee con `GET` global 0 y es el tempo inicial de la app. Se adopta **una sola vez**: después
  el tempo es de la app. El firmware estima el BPM de nuestros pulsos y lo mueve unos BPM, así que una
  relectura no puede pisarlo (hacía oscilar el número entre 140 y 142).
- Alternativa descartada: leer todos los proyectos cambiando de song. Cambia el estado del aparato y
  toca flash; no vale para arrancar.

### Disparar arranca si está parado

- `fire` manda el cambio con el transporte parado (carga inmediata) y recién después arranca. Si ya
  está sonando, sólo manda el cambio (el firmware lo aplica al compás siguiente).
- Un `Item::Project` con el transporte andando para, carga y vuelve a arrancar, porque el firmware
  sólo carga proyectos parado.
- El estado `playing` es el de la app (hilo de clock vivo), no una suposición por “enviado”.

### `fm1ctl` para verificar sin ojos

- `clock [int|usb|trs]` (lee o escribe, con `--dry`), `projects` y `song` dan la misma información
  que consume la app. Es la forma de comprobar la lectura y la restauración del reloj en la puerta.

## Risks / Trade-offs

- **La app se cuelga con `CLOCK = USB`** → el aparato queda esperando clock y su PLAY no avanza.
  Mitigación: restaurar en `Stop` y en `Drop`; documentar `fm1ctl clock int` como rescate. No hay
  forma de evitarlo del todo sin un watchdog en el firmware.
- **Jitter del hilo de clock** → spin en el último ms y deadline absoluto por pulso; el firmware
  valida ventanas de 62–375 ms por seis pulsos, así que tolera bastante.
- **Dos conexiones al mismo puerto** → probado con note-on y con section change durante el clock; si
  un firmware futuro lo rechaza, el error se muestra y no se inventa un OK.
- **El `SET` del reloj pisa un `CLOCK = TRS`** → se lee el valor original y se restaura ese, no un
  `INT` fijo.
- **Consulta y clock comparten el mismo aparato** → el clock se manda por su conexión y las
  consultas filtran por `cmd`; una respuesta de otra consulta vieja se descarta por timeout.
- **La lectura tarda** → hilo aparte y snapshot; la UI sigue viva con el set anterior.
- **El mapa NRPN documenta algo que el firmware no implementa** → los ids y rangos se usan como
  contrato de parámetros, pero la escritura va por el editor; conviene reportarlo al firmware.
- **Cargar una sección restaura sus globales** (BPM, `CLOCK`, FX, macros) → el reloj que se restaura al
  parar es el de la sección cargada, no el que tenía el aparato antes. Si una sección se guardó con
  `CLOCK = USB`, el stop la deja en USB. Mitigación: `fm1ctl clock int`; a futuro, guardar el reloj
  base al arrancar y refrescar las franjas después de un disparo.

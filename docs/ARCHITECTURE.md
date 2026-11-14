---
id: architecture
status: vigente
version: 0.1.0
updated: 2026-11-13
---

# Arquitectura

## Capas y regla de dependencia

```
        apps (entrega)                 fm1ctl · fm1-perf (egui)
             │  usan
             ▼
     adaptador (infraestructura)       fm1-midi (midir / CoreMIDI) · fm1-store (JSON en disco) · fm1-flash (OTA)
             │  cumple los traits
             ▼
   dominio (reglas, sin I/O)           fm1-core: nrpn · message · editor · profile · device · setlist
```

Las flechas del código apuntan hacia adentro. `fm1-core` declara `MidiOut` y `MidiIn` en
`transport.rs`; `fm1-midi` los implementa con `midir`. Ninguna regla del dominio conoce CoreMIDI, un
nombre de puerto ni un archivo.

Lo que esto compra, concretamente: los 62 tests corren sin aparato y sin pantalla, y el `--dry` de la
CLI verifica la codificación de bytes en un segundo.

## Módulos

| módulo | hace | no hace |
|---|---|---|
| `fm1-core/src/nrpn.rs` | espacios de parámetros (pista/global), búsqueda por nombre o id, escalado a 14 bits, E1..E8 por motor | no abre puertos ni sabe de canales MIDI reales |
| `fm1-core/src/nrpn_table.rs` | **generado** desde `docs/NRPN.md` (239 parámetros: 80 de pista, 96 de motor, 63 globales) | no se edita a mano |
| `fm1-core/src/message.rs` | bytes: CC, PC, notas, NRPN de 4 mensajes, realtime (`Start`/`Stop`/clock), handshake de identidad | no tiene estado ni decide |
| `fm1-core/src/editor.rs` | editor protocol: tramas `F0 7D 46 4C …`, offset binary, `Clock`, parseo de `INFO`/`GET`/`SET`/`PROJECT`/`SONG` | no abre puertos ni espera respuestas |
| `fm1-core/src/profile.rs` | perfil de firmware: identidad, `Param`/`ParamAccess` (cc/nrpn/editor), transporte, escenas y built-ins `generic`/`jiant-par12`/`x0x-1.2` | no abre puertos ni archivos |
| `fm1-core/src/device.rs` | intenciones: `set_bpm`, `set_global`, `set_track`, `goto_section`, `load_project`, `load_sound`, `play(item)`, `panic`, `set_clock`, `start`/`stop`/`pulse` | no sabe si abajo hay USB, TRS o un grabador de prueba |
| `fm1-core/src/setlist.rs` | modelo serializable del set (sección, proyecto, sonido) con validación de rangos | no persiste: eso es del adaptador |
| `fm1-core/src/route.rs` | la ruta de instalación: familia del aparato y del paquete, un paso directo con aviso entre forks, negativas explícitas (stock por M-UPGRADE) | no escribe, no abre puertos, no hashea nada |
| `fm1-core/src/transport.rs` | los traits `MidiOut`/`MidiIn` y los dobles `Recorder`, `ScriptedIn`, `Silent` | no transporta nada |
| `fm1-midi/src/lib.rs` | descubre puertos, abre salida, lee en un hilo, `identify()`, `editor()` (pedido/respuesta), `read/write_param` por acceso (editor/CC/NRPN), `watch`, `ClockMaster` (reloj USB o MIDI) | no decide qué mandar ni arma el set |
| `fm1-store/src/lib.rs` | setlist, `panel.json` y perfiles de usuario en JSON versionado, escritura atómica, validación, `.bak` | no sabe de MIDI, de egui ni de puertos |
| `fm1-store/src/catalog.rs` | catálogo de firmwares (built-in embebido + usuario), resolución de rutas, SHA-256 de los paquetes, backup de config/paquetes | no decide cuándo ni cómo flashear |
| `fm1-store/src/sha256.rs` | SHA-256 mínimo (memoria y archivo) con vectores conocidos | no es criptografía de secretos |
| `fm1-flash/src/lib.rs` | instalador OTA/Felucca: parseo del `.fwsc`, handshake, loader, responder pedidos de lectura, verificar identidad al reiniciar | no decide ni confirma: eso es de la app/CLI |
| `fm1ctl/src/main.rs` | CLI: puertos, identidad, reloj, proyectos, cadena, mensajes crudos, intenciones, `play`, `setlist`, `firmware`, `flash`, `--dry` | no dibuja |
| `fm1-perf/src/app.rs` | estado de la sesión (perfil, franjas asignadas, transporte, historial), cambio de perfil, picker, teclado, layout, `--shot`/`--read` | no abre puertos ni sabe de bytes |
| `fm1-perf/src/library.rs` | ventana de biblioteca: búsqueda, estado de paquetes, identidad instalada, confirmación, backup y flash en un hilo | no dibuja el panel ni conoce el protocolo OTA |
| `fm1-perf/src/profiles.rs` | selector de perfil, armado de franjas desde `panel.json` y carga del setlist por perfil | no habla MIDI ni abre puertos |
| `fm1-perf/src/link.rs` | el único punto que habla con `fm1-midi`: detección de perfil, sondeo, lectura de parámetros asignados, `Monitor` de entrada, envío de intenciones y reloj | no dibuja ni guarda estado |
| `fm1-perf/src/theme.rs` | tokens de «Ceniza» (los de `DESIGN.md`) y `Visuals` de egui | no decide layout |
| `fm1-perf/src/panels/*` | setlist, escenario y cabecera/pie: cada panel dibuja en el `Rect` que le dan | no deciden el tamaño del otro |
| `fm1-perf/src/widgets.rs` | pelo de 1 px, regla de marcas, franja de parámetro | no conoce el dominio |

## Perfiles

La app no habla un firmware: habla el **perfil** que elige al arrancar. Un perfil declara:

- **Identidad**: subcadenas de la identidad OTA / INFO para reconocerlo.
- **Parámetros**: `id`, `label`, `scope`, rango, default y **acceso**: `cc { channel, cc }`,
  `nrpn { channel, msb, lsb }`, `editor { scope, id }` (JIANT) o `none`.
- **Transporte**: `clock_master` (Start + clock; opcionalmente setea `CLOCK` por editor), `midi` o
  `none`.
- **Escenas**: `program_change` (secciones/proyectos), `patterns`, `clips` o `none`. La UI de setlist y
  las teclas `1–6`/`s`/`l`/`c` sólo aparecen si hay escenas.

Built-in: `generic` (66 CC de 6 pistas), `jiant-par12` (editor, secciones, reloj USB) y `x0x-1.2`
(112 CC: 909/808/303/break, canales 10/11/2/3/4). Los perfiles de usuario van en
`~/.config/fm1lab/profiles/*.json`; el de usuario gana por `id`. La detección usa la identidad OTA +
`INFO`; si no matchea, cae a `generic`; `--profile` lo fuerza. `p` abre el selector y `fm1ctl profile`
lo lista; la elección manual se recuerda en `profile.json` y se aplica cuando no hay aparato.

La capa de acceso es única: `Port::read_param`/`write_param` eligen el protocolo del `ParamAccess`. El
editor confirma cada escritura; CC/NRPN no tienen ACK. El sondeo (`Probe`) sigue sólo los parámetros
legibles (editor); los CC/NRPN se llenan con el monitor/learn o con el último valor escrito. Las
franjas se asignan con `a` (picker) y se guardan por `profile_id` en `panel.json`.

## El protocolo, en una tabla

| intención | mensaje | canal |
|---|---|---|
| tempo, swing, DJ filter, reverb | editor `SET` global (id del mapa, offset binary) | — |
| parámetro de una pista | editor `SET` scope 0 (la pista seleccionada) | — |
| valor de un parámetro o global | editor `GET` | — |
| level, pan, cutoff, res, sends | CC 7, 10, 74, 71, 91/93/94 | el de la pista |
| los ocho EDIT del motor | CC 16..23 | el de la pista |
| sección A–D | Program Change 0–3 | 16 |
| proyecto 1–6 | Program Change 16–21 | 16 |
| sonido de la lista PRESETS | Program Change n | el de la pista |
| identidad del aparato | SysEx `F0 00 32 45 00 00 00 40 7F F7` (lectura) | — |
| transporte desde la app | realtime `FA` (start), `FC` (stop), `F8` (clock, 24/negra) | — |
| fuente de reloj | editor `SET` global 2 `CLOCK` (0 INT, 1 USB, 2 TRS) | — |
| sección con música / cadena | editor `PROJECT` op 2 / `SONG` op 0 | — |
| tempo y reloj actuales | editor `GET` global 0 / global 2 | — |

**Ojo con los dos protocolos.** El NRPN escala sobre el rango del parámetro
(`v = min + (x*(max-min) + 8191)/16383`); el editor protocol usa **offset binary**
(`valor + 8192`) y va en tramas `F0 7D 46 4C cmd args F7`. El mismo número no significa lo mismo en
los dos. En par12 el firmware **no implementa NRPN** (CC99/98 sólo cancelan el RPN): la app escribe
tempo, franjas y reloj por el editor, con los ids y rangos del mapa. Fuente: manual del firmware (§14),
`docs/NRPN.md` y `web/EDITOR_PROTOCOL.md`.

**El reloj manda.** Con `CLOCK = INT` el firmware ignora `Start`/`Stop`/clock por USB: sólo los atiende
si `G_CLOCK == 1` (USB). Por eso el play de la app lee el reloj, lo pone en USB, manda `Start` y el
clock, y al parar restaura el valor original.

## Persistencia

`fm1-store` guarda el setlist en `~/.config/fm1lab/setlist.json` (o `FM1LAB_SETLIST` / `--setlist`).
Formato: `{"version":1,"name":"…","items":[…]}`; el `Item` conserva su `kind`/`value`. Al cargar se
valida cada paso con `Setlist::validate()` y se rechaza una versión desconocida. El guardado es
atómico: se escribe `<archivo>.tmp` en el mismo directorio, `sync_all` y `rename`; si algo falla, el
original queda y el temporal se borra. Un JSON roto se mueve a `<archivo>.bak` y se devuelve el error
(no se inventa un set). La plantilla A–D se escribe con `fm1ctl setlist init`.

La app arranca con el archivo si existe; en ese caso el sondeo no reemplaza el set. `r` pasa al set del
aparato y `s` guarda el actual. Sin archivo, el origen es el aparato, como en M1.

`panel.json` guarda las franjas asignadas por `profile_id`, `~/.config/fm1lab/profiles/*.json` los
perfiles de usuario y `~/.config/fm1lab/profile.json` el último perfil elegido (`FM1LAB_LAST_PROFILE`);
los tres usan la misma escritura atómica y el mismo `.bak`. La app prefiere la identidad detectada; si
no hay aparato, arranca con el último perfil y no salta a `generic`.

## Biblioteca e instalación

El catálogo es dato, no código: `firmwares.json` en la raíz viaja embebido (`include_str!`) y el
usuario puede sumar o pisar entradas en `~/.config/fm1lab/firmwares.json` (`FM1LAB_CATALOG`). Cada
entrada declara `id`, nombre, versión, familia, identidad OTA, archivo, SHA-256, etiquetas, notas y
perfil sugerido. Los paquetes se resuelven contra `FM1LAB_FIRMWARES`, o
`~/Proyectos/m-vave/firmware`, o `~/.config/fm1lab/firmwares`.

`fm1-store::catalog` verifica el SHA-256 con `sha256.rs` (implementación mínima, sin dependencias) y
`backup_lab()` copia setlist, panel, catálogo, perfiles y todos los `.fwsc` a
`~/.config/fm1lab/backups/<fecha>/`. La ventana (`fm1-perf/src/library.rs`) muestra el estado y, al
confirmar, delega en `fm1-flash`; la app para transporte, `Probe` y `Monitor` para que nadie más toque
el aparato durante el OTA. `fm1-flash` no decide: recibe un `Package`, corre el handshake, responde
los pedidos de lectura del loader y verifica la identidad final. Al terminar, la app re-detecta el
perfil y lo cambia en caliente (sin reiniciar); si el aparato tarda en enumerar, avisa. El primer flash
real es la única verificación pendiente; el camino de rescate sigue siendo el instalador Python de
`m-vave`.

## Seguimiento

El firmware JIANT **no manda eco de NRPN**: su MIDI OUT sólo emite notas y clock. El hilo del `Probe`
consulta los parámetros **asignados** que el perfil declare legibles (`editor`) cada ~500 ms y publica
un `Snapshot` con la pista seleccionada y los valores; la app lo aplica a las franjas sin tocar el set
ni el tempo. Un `↑↓` recién movido (400 ms) no se pisa con un sondeo viejo.

Para CC/NRPN no hay lectura estándar: un `Monitor` aparte escucha el MIDI de entrada, actualiza las
franjas asignadas cuando el aparato manda el movimiento y, con `k`/«aprender», convierte el próximo
CC/NRPN en un parámetro del perfil de usuario (id `cc.ch.cc` o `nrpn.ch:msb:lsb`), lo asigna a la
franja y lo guarda en `profiles/<id>.json` y `panel.json`. `fm1ctl mix` sigue imprimiendo la foto
JIANT de cuatro ids.

## Lo que está generado, y por qué

`docs/NRPN.md` → `crates/fm1-core/src/nrpn_table.rs`, por `scripts/gen_nrpn.py`. Es el mismo trato que
hace el firmware con su documentación: la tabla no se escribe a mano para que no pueda mentir. Un
`--check` en la puerta de CI detecta el desfase.

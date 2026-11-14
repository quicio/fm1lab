---
id: handoff
status: vigente
version: 0.1.0
updated: 2026-11-13
---

# Handoff

## Estado medido (2026-11-13)

| cosa | número | comando |
|---|---|---|
| crates | 6 (`fm1-core`, `fm1-midi`, `fm1-store`, `fm1-flash`, `fm1ctl`, `fm1-perf`) | `cargo metadata --no-deps` |
| app de UI | 4.042 líneas de Rust, binario `fast` de 7,4 MB | `wc -l crates/fm1-perf/src/*.rs crates/fm1-perf/src/panels/*.rs` |
| tests | **75**, en verde, sin aparato (9 de la ruta de instalación) | `cargo test` |
| perfiles built-in | 4 (`generic` 66 CC · `jiant-par12` 137 editor · `x0x-1.2` 112 CC · `sloop-2.8` 44 CC) | `fm1-perf --read` / `cargo test -p fm1-core` |
| firmwares en el catálogo | **11** (stock, JIANT 0.9.9 y V10, X0X, dock, SLOOP, CTL-1, Felucca, sidou), todos con SHA-256 | `fm1ctl firmware` |
| parámetros en el mapa JIANT | **239** (80 pista · 96 motor · 63 globales) | `python3 scripts/gen_nrpn.py` |
| clippy | limpio con `-D warnings` | `cargo clippy --all-targets -- -D warnings` |
| puerta completa | verde | `scripts/check.sh` |
| líneas de Rust | 9.965 (incluye la tabla generada) | `find crates -name '*.rs' \| xargs wc -l` |
| disco libre | 7,9 GB; `target/` 1,0 GB | `df -h /`; `du -sh target` |

El aparato corre **X0X 1.2.2** (`FM-1_9010202`, puerto `Felucca`) desde el 2026-11-13. El perfil
`jiant-par12` sigue soportado, pero **no hay prueba en vivo de JIANT con esta flash**: para eso hay que
volver a JIANT con los `.fwsc` de `~/Proyectos/m-vave/firmware/`.

Lo que **no** está hecho: perfil de TRIBE, `params --profile` y la prueba en vivo de
learn/transporte. El monitor/learn ya está (`m`/`k`), la biblioteca de firmwares también (`f`) y el
selector de perfil en la ventana (`p`). El **instalador Rust sólo se probó en seco**: el primer flash
real queda pendiente de una confirmación explícita.

### La app en dos comandos

```bash
cargo run --profile fast -p fm1-perf                                  # la ventana
cargo run --profile fast -p fm1-perf -- --shot shots/panel.png --demo  # captura y sale
cargo run --profile fast -p fm1-perf -- --read                        # perfil + params/set, sin ventana
cargo run --profile fast -p fm1-perf -- --profile x0x-1.2             # fuerza un perfil
cargo run --profile fast -p fm1-perf -- --mix 5                       # sigue las franjas 5 s
cargo run --profile fast -p fm1-perf -- --setlist show.json           # otro archivo de setlist (JIANT)
cargo run --profile fast -p fm1-perf -- --library                     # abre la biblioteca al arrancar
cargo run --profile fast -p fm1-perf -- --profiles                    # abre el selector de perfil
cargo run --profile fast -p fm1-perf -- --shot shots/firmware.png --demo --library
cargo run --profile fast -p fm1-perf -- --shot shots/profiles.png --demo --profiles
cargo run --profile fast -p fm1ctl -- profile                          # perfiles y el detectado
cargo run --profile fast -p fm1ctl -- profile x0x-1.2                  # detalle de un perfil
cargo run --profile fast -p fm1-perf -- --shot shots/route-jiant.png --library --firmware jiant-v10.7.23
```

### Distribución macOS

`scripts/make_app.sh` arma `FM1Lab.app` con `fm1-perf` como ejecutable, `fm1ctl` al lado y el ícono
`assets/fm1lab.icns` (fuente editable en `assets/icon.svg`, render con `rsvg-convert` + `make_icns.sh`). El workflow
`.github/workflows/release.yml` corre la puerta, compila `dist` para arm64 e x86_64, empaqueta el `.app`
y publica en el release del tag `vX.Y.Z`:

- `fm1lab-macos-arm64-app.tar.gz` / `fm1lab-macos-x86_64-app.tar.gz` (alias estable para el cask)
- `fm1lab-X.Y.Z-macos-{arm64,x86_64}-app.tar.gz` (versionados) y `SHA256SUMS.txt`

El cask estático vive en `Casks/fm1lab.rb` y en `quicio/homebrew-tap`; apunta a
`releases/latest/download/…`, así que no hay que bumpearlo en cada release:

```bash
brew install --cask quicio/tap/fm1lab
open -a FM1Lab
fm1ctl ports
```

Teclas comunes: `espacio` play/stop · `↑↓` elegir franja · `←→` mover valor (`shift` de a 8) · `±`
tempo (`⌥` de a 5) · `a` asignar un parámetro a la franja · `m` monitor MIDI · `k` aprender un
CC/NRPN · `p` selector de perfil · `f` biblioteca de firmwares · `esc` pánico · `r` releer. Con el perfil JIANT además: `1–6`
disparar · `s` guardar el set · `l` cargarlo · `c` ciclar el reloj (estas dos últimas no están en el
pie por espacio).

### Perfiles

El lab detecta el firmware por identidad OTA + INFO y elige un perfil; si no lo reconoce, usa
`generic`. `--profile <id>` lo fuerza. Un perfil declara parámetros con su acceso (`cc`, `nrpn`,
`editor`), transporte y escenas. Built-in:

- `generic`: 66 parámetros CC (6 pistas × level, pan, cutoff, res, atk, dec, rel, glide, rev, chorus,
  delay). Transporte MIDI genérico; sin escenas. Es el fallback para un firmware desconocido.
- `jiant-par12`: el editor SysEx, secciones/proyectos, reloj USB y la lectura real del set; es el
  perfil que usan los documentos de más abajo cuando hablan de JIANT.
- `x0x-1.2`: 112 parámetros CC del manual de X0X (909 canal 10, 808 canal 11, 303A 2, 303B 3, break 4;
  level/pan/rev/dly y los parámetros de cada parte). Transporte MIDI; X0X sigue el clock externo solo.
- `sloop-2.8`: 44 parámetros CC del README de SLOOP, 4 pistas × 11 (level, pan, filter, reso, atk, dec,
  rel, glide, rev, chorus, delay). Canales por defecto: drums 10, bass 2, pad 11, lead 8; transporte MIDI
  (SLOOP sigue el clock externo con GLO > SYSTEM > SYNC = USB).

Los perfiles de usuario viven en `~/.config/fm1lab/profiles/*.json` (o `FM1LAB_PROFILES`) y ganan por
`id`. Las franjas asignadas se guardan en `~/.config/fm1lab/panel.json` (o `FM1LAB_PANEL`), por perfil.

`p` abre el selector con built-ins + usuario y cambia en caliente: franjas, setlist, sondeo y
transporte. La elección se guarda en `~/.config/fm1lab/profile.json` (`FM1LAB_LAST_PROFILE`) y se usa al
arrancar cuando no hay aparato; si hay aparato, manda la identidad detectada. `fm1ctl profile` lista lo
mismo en la terminal y `fm1ctl profile <id>` muestra params, transporte, escenas y franjas.

### Setlist en disco (perfil JIANT)

`~/.config/fm1lab/setlist.json`, o `FM1LAB_SETLIST` / `--setlist <archivo>`. JSON versionado
(`{"version":1,"name":...,"items":[...]}`), escritura atómica (temporal + `rename`), validación de
rangos al cargar y copia `.bak` si el archivo está corrupto. Al arrancar con archivo, el set es el
archivo y el sondeo no lo pisa; `r` trae el set del aparato y `s` guarda. Sin archivo, la app usa el
set del aparato (como antes).

### Seguimiento del aparato

Con el perfil JIANT, el hilo de sondeo lee los parámetros **asignados** que el editor permite leer
(GET) cada ~500 ms; la UI aplica el snapshot sin tocar el set ni el tempo. Un `↑↓` recién movido
(400 ms) no se pisa con un sondeo viejo. Con un parámetro CC/NRPN no hay lectura estándar: la franja
muestra el último valor escrito y el **monitor/learn** (fase 2) lo llenará desde el MIDI de entrada.

Con el perfil genérico no hay setlist ni reloj de JIANT: `espacio` manda Start + clock MIDI y la
configuración de reloj del aparato queda a cargo del firmware.

### Monitor y learn

`m` abre el monitor con los últimos CC/NRPN que manda el aparato. `k` (o el botón **aprender** del
monitor) arma la captura: el próximo CC o NRPN que llegue se convierte en un parámetro
(`cc.<canal>.<cc>` o `nrpn.<canal>.<msb>:<lsb>`), se asigna a la franja enfocada y se guarda en el
perfil de usuario (`~/.config/fm1lab/profiles/<id>.json`) y en `panel.json`. Así se mapea un firmware
desconocido sin editar JSON. El monitor también actualiza las franjas CC/NRPN cuando el aparato manda
el movimiento de una perilla. En X0X hay que poner `GLO > MIDI > CC OUT = KNOBS` (o ALL) para que las
perillas salgan; las escrituras CC del perfil funcionan sin ese ajuste. Los canales por defecto son
909=10, 808=11, 303A=2, 303B=3, break=4.

### Biblioteca e instalación de firmwares

`f` abre la **biblioteca**: buscador por nombre/familia/versión/etiqueta, estado de cada paquete
(`falta`, `sha256 no coincide`, `listo`) y qué firmware reporta el aparato por identidad OTA. El
catálogo built-in es `firmwares.json` en la raíz del repo y viaja embebido en el binario; el del
usuario es `~/.config/fm1lab/firmwares.json` (o `FM1LAB_CATALOG`) y gana por `id`. Los paquetes se
resuelven contra `FM1LAB_FIRMWARES`, o `~/Proyectos/m-vave/firmware` si existe, o
`~/.config/fm1lab/firmwares`.

Instalar exige **confirmación explícita** en la ventana (o `--yes` en la CLI). El flujo, en este
orden: se para el transporte, se pausan el sondeo y el monitor, se copia `setlist.json`, `panel.json`,
`firmwares.json`, los perfiles y todos los `.fwsc` a `~/.config/fm1lab/backups/<AAAAMMDD-HHMMSS>/`,
se verifica el SHA-256 y recién ahí `fm1-flash` habla el protocolo OTA/Felucca (handshake, loader,
escritura y verificación de identidad al reiniciar). El stock V16 no trae el loader de Felucca y **no se
puede instalar por OTA**: se rechaza y se manda a M-UPGRADE. No hay instalación silenciosa.

Al terminar el flash la app **re-detecta el perfil** sin reiniciar; si el aparato tarda en volver a
enumerar, el pie avisa y un reinicio lo resuelve. La CLI equivalente es `fm1ctl firmware` (lista), `fm1ctl firmware --install <id> --dry` (valida
sin escribir) y `fm1ctl firmware --install <id> --yes`; `fm1ctl flash <archivo> [puerto]` sigue
existiendo para un paquete suelto.

Al disparar con el transporte parado, la app primero carga la sección/proyecto y después arranca:
el primer compás es el paso disparado. Con el transporte andando, el cambio entra al compás siguiente
(lo decide el firmware). El tempo se adopta del aparato **una sola vez** al arrancar; después manda la
app y `r` no lo pisa (el firmware estima el BPM de nuestros pulsos y tiembla).

## Verificación sin ojos

1. `scripts/check.sh` — fmt, clippy, tests, mapa al día y humo de bytes. Es la definición de terminado.
2. `fm1ctl ... --dry` — imprime los bytes y **no** abre puertos. Es cómo se verifica la codificación sin
   aparato; el humo del `check.sh` compara contra valores que ya se validaron a mano:
   `bpm 140` → `F0 7D 46 4C 03 01 00 0C 41 F7` (editor, offset binary), `section C` → `CF 02`,
   `project 3` → `CF 12`, `clock usb` → `F0 7D 46 4C 03 01 02 01 40 F7`.
3. `fm1ctl ports` / `fm1ctl info` — sólo con aparato: puertos visibles e identidad por handshake
   (`FM-1_909` en JIANT 0.9.9-par12; stock V15 y V16 también contestan).
4. `fm1ctl clock` / `projects` / `song` / `mix` — lo mismo que lee la app, por la terminal.
   `fm1-perf --read` imprime el set que armaría sin abrir la ventana.
5. `fm1ctl play [seg]` — arranca el transporte como reloj maestro y lo para; con el audio USB del
   aparato se verifica que suene y que el reloj vuelva a su valor. Es el humo con aparato del
   transporte.
6. `fm1ctl setlist init <archivo>` / `fm1ctl setlist <archivo> [--dry]` — escribe la plantilla, valida
   y lista un setlist; con `--dry` imprime los bytes de cada paso sin abrir puerto. Es el humo de M2.
7. `fm1-perf --mix 5` — arranca el hilo de seguimiento sin ventana e imprime la foto de las franjas
   cuando cambia. Cambiar `SIZE` por el editor (o girar la perilla) y ver el valor nuevo es el humo
   de M3.
8. `fm1-perf --play-close 2` — arranca el reloj, cierra la ventana a los 2 s y sale. Después,
   `fm1ctl clock` tiene que decir `INT`: es el humo del cierre normal (la X).
9. `fm1ctl firmware [query]` — lista el catálogo con identidad, SHA-256 y estado del paquete; con
   aparato marca con `*` el que está instalado. `fm1ctl firmware --install <id> --dry` valida el
   paquete sin abrir puerto; `--yes` es el único camino que escribe. `fm1-perf --shot shots/firmware.png
   --demo --library` verifica el dibujo de la ventana.
10. `fm1ctl profile` / `profile <id>` — perfiles disponibles, el detectado y el detalle; es el mismo
    catálogo que usa `p` en la ventana.

## Invariantes que no se rompen

- `fm1-core` no importa `midir` ni `eframe`. Si aparece, la capa está filtrada.
- `nrpn_table.rs` es salida de `scripts/gen_nrpn.py`. Editarlo a mano rompe `--check`.
- **El mapa NRPN da ids y rangos, pero par12 no implementa NRPN**: en el firmware, CC99/98 sólo
  cancelan el RPN y CC6/38 escriben el bend range. La app escribe tempo, franjas y reloj por el
  **editor protocol** (offset binary, ids del mapa, valores crudos). `fm1ctl param` sigue mandando
  NRPN como inspección de bytes, no como camino que mueva el aparato.
- El **editor protocol** va en offset binary (`valor + 8192`), no escalado. Son dos caminos distintos:
  NRPN para parámetros, editor para `CLOCK`, proyectos y cadenas. Mezclarlos escribe otro valor.
- El **reloj manda**: con `CLOCK = INT` el firmware ignora `Start`/`Stop`/clock por USB. Si el
  transporte no arranca, revisar `fm1ctl clock`, no el mensaje. La app lo pone en `USB` al dar play y
  restaura el valor original al parar.
- El canal importa: un NRPN de pista sólo lo escucha la pista que toca ese canal. Si no se mueve nada,
  lo primero a revisar es el canal y el ROUTE, no el mensaje.
- La app nunca bloquea el hilo de dibujo esperando al aparato: lectura en su hilo, envío best-effort.
- **Cerrar la ventana no es `Drop`**: eframe puede salir sin correr destructores. El reloj se para y
  restaura en `on_exit` y al ver `close_requested`; `--play-close 2` es la prueba (arranca, cierra y
  deja `clock = INT`). Si el proceso muere por kill/Ctrl-C/Cmd-Q, queda `fm1ctl clock int` como
  rescate.

## Qué NO hacer

- **No subir egui/eframe** sin ADR: 0.37 es una reescritura (`App::ui`, sin `TopBottomPanel`).
- **No agregar un webview** para la UI. La decisión está en ADR-0002 y es lo que mantiene el binario chico.
- **No sumar `incluir` un segundo árbol de specs**: `openspec/` es la fuente (ADR-0005).
- **No copiar `candil/src/app.rs` como forma**: 10.473 líneas en un archivo es exactamente lo que este
  proyecto evita.
- **No prometer ACK**: el aparato no confirma nada. Toda confirmación sale del eco (MIDI OUT ALL) o del
  editor protocol.
- **No mandar `Start` esperando que arranque con `CLOCK = INT`**: el firmware lo descarta. Play desde la
  app = `CLOCK = USB` + clock; si no, se arranca con PLAY en el aparato.
- **No flashear sin backup, sin SHA-256 y sin confirmación.** `fm1-flash` es el único camino de
  escritura; la app para transporte/sondeo/monitor y respalda config y paquetes antes. El instalador
  Python de `m-vave` queda como rescate si el OTA se corta. Si dudás, `fm1ctl clock int` y reinstalar
  el stock/JIANT con M-UPGRADE.

## Prueba en vivo (2026-11-11, FM-1_909 · JIANT 0.9.9-par12)

Ida: `fm1ctl` abrió el puerto `Felucca`, leyó la identidad por handshake y envió `bpm 140`, `section B`,
`section A`, `project 2` y `CC74` en el canal 1, todos aceptados.

Vuelta: escuchando el puerto 90 s mientras el aparato tocaba (PLAY), llegaron **96 mensajes**:

| medida | valor |
|---|---|
| note-on / note-off | 48 / 48 (balance 0: sin notas colgadas) |
| canales | sólo el 1 |
| alturas | 23..55 (16 distintas) |
| velocidad | 100 constante (pasos del secuenciador, no ejecución humana) |
| otros mensajes (CC, clock, SysEx) | ninguno |

Consecuencias operativas, ya incorporadas al diseño:

- El aparato **no confirma** lo que recibe: la única verificación de la vuelta es que emita. Por eso la UI
  tiene que ser optimista y reconciliar por el eco.
- Las notas del secuenciador salieron ⇒ el ajuste `MENU > CONTROL > MIDI OUT` está en `ALL`. En `KEYS`
  (default) sólo salen las teclas, y en `OFF`/`CLOCK` no hay notas: una ventana muda no es una falla del
  cliente.
- No hubo bytes de clock: con `ALL` el reloj no se manda. Si la app necesita seguir el tempo, hay que
  pedirle al usuario `MIDI OUT = CLOCK` (o seguir el reloj por otra vía).

## Prueba en vivo (2026-11-12, FM-1_909 · par12)

- El proyecto cargado tenía las secciones **A, B y C** guardadas y **D vacía**; la cadena, A(8) B(8)
  A(8) C(8). `fm1-perf --read` devolvió `set: sección A · sección B · sección A · sección C` y el BPM
  real del aparato. Las secciones vacías no entran al set.
- `fm1ctl play 4` a 142 BPM: el audio USB del aparato mostró −25 dB durante la reproducción y silencio
  al parar; al terminar, `fm1ctl clock` volvió a `INT`. El `Start` solo, con `CLOCK = INT`, no hace nada
  (el firmware lo descarta); con `USB` + clock, sí.
- `fm1ctl projects` / `song` coinciden con lo que lee la app. La lectura tarda menos de un segundo y no
  bloquea la ventana.

## Cambio de firmware (2026-11-13)

- Se instaló **X0X 1.2.2** (`FM-1_9010202`, puerto `Felucca`) con el instalador de `m-vave`
  (`x0x/fm1_install.py`); el proyecto/sonidos de JIANT se borraron con la flash.
- La app detecta **`x0x-1.2`** (identidad `FM-1_9010202`) y muestra 112 parámetros CC (909/808/303/
  break); `fm1-perf --read` imprime el perfil. `generic` queda como fallback. La prueba en vivo de
  JIANT queda pendiente hasta volver a instalarlo.
- Rollback: `firmware/fm1-stock-official.fwsc` (V16) y `jiant-…-par12.fwsc`, con los instaladores de
  `m-vave/jiant` y `m-vave/x0x`. El `CLOCK` de X0X se configura en su propio menú.

## Biblioteca de firmwares (2026-11-13, verificación en seco)

- El catálogo built-in tiene 11 paquetes con SHA-256 calculado con `shasum -a 256`; `state_of` los
  verifica con un SHA-256 propio (vectores conocidos en `cargo test`).
- `fm1ctl flash --dry` parseó X0X (`FM-1_9010202`, 609.662 bytes), JIANT par12 (`FM-1_909`,
  610.260), dock (`FM-1_921`, 610.110) y stock (`FM-1_016`, 718.176; sin loader, sólo M-UPGRADE).
- `fm1ctl firmware` listó los 11 con su estado y `fm1ctl firmware --install x0x-1.2.2 --dry` validó el
  paquete. El instalador Rust se estrenó en vivo el 2026-11-14 con el cambio directo X0X → SLOOP.
- El aparato no tenía puertos MIDI visibles al cerrar la sesión, así que la ventana mostró
  «sin respuesta»: es el comportamiento honesto, no un fallo de la biblioteca.

## Ruta de instalación y cambio real (2026-11-14)

El intento de cambiar X0X → SLOOP por la ruta de ADR-0010 falló en el **paso al oficial**:

```
flash: el FM-1 dejó de contestar después de 4 pedidos; no se escribió nada
```

`fm1-stock-official.fwsc` no es un paquete Felucca (no tiene `FELUCCA-LOADER-1`): el X0X corta la lectura
del paquete y no hay forma de usar el stock como paso intermedio. El stock sólo entra con **M-UPGRADE**.

El cambio directo, en una sola escritura, **funcionó**:

```bash
fm1ctl flash firmware/sloop-2.8.5.fwsc --straight --yes
# ...
# escritura completa
# aparato: FM-1_900 (Felucca)
```

El primer intento dio dos falsos errores por timeouts cortos: el loader tardó más de 30 s en enumerar y el
aparato tardó más de 40 s en volver. Se subieron `wait_loader` y `wait_reboot` a **120 s** (ADR-0011).

La ruta queda así (ADR-0011, reemplaza a ADR-0010):

```
$ fm1ctl route jiant-v10.7.23
aparato: FM-1_900 (sloop)
paquete: jiant-v10.7.23 (jiant, identidad FM-1_909)
ruta: 1 paso
  1. destino -> espera FM-1_909
     cambio directo entre forks: la zona de datos puede quedar incompatible; exportá los proyectos antes
se pierde: los proyectos y presets del firmware actual
```

- `route stock-v16` se niega: «el paquete oficial V16 no es un paquete Felucca… usá M-UPGRADE».
- `route x0x-1.2.2` se niega si ya está esa identidad; `route jiant-v10.7.23` desde SLOOP es 1 paso.
- La decisión vive en `fm1-core::route`; `route` corre sin escribir y acepta `--from IDENT`.
- La **biblioteca de la UI** (`f`) muestra la ruta y el aviso de datos; instalar es un solo clic. `--firmware
  <id>` abre la biblioteca con esa entrada seleccionada.
- SLOOP tiene perfil **`sloop-2.8`**: 4 pistas × 11 CC (level, pan, filter, reso, atk, dec, rel, glide, rev,
  chorus, delay). Canales por defecto: drums 10, bass 2, pad 11, lead 8; si se cambian en GLO > MIDI, se
  vuelven a aprender con `k`.

## Deuda conocida

| causa | arreglo |
|---|---|
| El cambio entre forks puede dejar la zona de datos incompatible | el aviso está antes de confirmar; el rescate es M-UPGRADE o el instalador Python de `m-vave` |
| El stock V16 no se puede instalar por OTA | usar M-UPGRADE; la app lo rechaza con ese mensaje |
| La biblioteca no consulta red: sólo built-in + usuario | un catálogo remoto firmado, si algún día hace falta |
| `Installer::probe` abre el puerto en cada apertura de la biblioteca mientras el sondeo/monitor también leen | cachear la identidad o reusar el `Probe`; hoy el flash los pausa, la lectura no |
| La re-detección tras un flash depende de que el aparato vuelva a enumerar el puerto | la app lo intenta al terminar; si falla, reiniciar `fm1lab` |
| Falta el perfil de TRIBE | el monitor/learn (`m`/`k`) ya permite construirlo en vivo |
| JIANT no tiene prueba en vivo con X0X instalado | volver a JIANT con los `.fwsc` guardados y correr el humo de JIANT |
| La CLI paniquea con *broken pipe* si la salida se corta (`fm1ctl params \| head`): `println!` entra en pánico con EPIPE | escribir por `io::stdout()` ignorando `ErrorKind::BrokenPipe`; el `check.sh` lo esquiva con `grep` sin `-q` |
| Sin hot-plug: si el aparato se desconecta, `send` empieza a fallar; el sondeo lo detecta pero no avisa al instante | el sondeo relee al aparecer y el transporte se corta con error; falta un botón de reconectar explícito |
| `Device::identify` (en el core) y `Port::identify` (en el adaptador) se solapan | dejar uno solo cuando la app de UI defina de quién es la responsabilidad |
| Si el proceso muere sin `Drop` (kill, Ctrl-C, Cmd-Q), el aparato queda en `CLOCK = USB` y su PLAY no avanza | `fm1ctl clock int` lo devuelve; cerrar con la X lo restaura (`on_exit`/`close_requested`, probado con `--play-close`) |
| El firmware estima el BPM del clock y lo guarda con unos BPM de error | al parar la app deja el BPM en el tempo que usó; la próxima lectura arranca con ese valor |
| Cargar una sección restaura sus globales (BPM, `CLOCK`, SIZE, MIX) y la UI no lo refleja hasta `r`; el reloj que se restaura al parar es el de la sección | releer con `r`; a futuro, guardar el reloj base al arrancar y refrescar las franjas tras un disparo |
| El listener no imprime tiempos: sirve para ver mensajes, no para medir tempo ni jitter | imprimir `Instant`/stamp por mensaje y calcular el intervalo entre pasos |

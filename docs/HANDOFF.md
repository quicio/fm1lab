---
id: handoff
status: vigente
version: 0.1.0
updated: 2026-11-11
---

# Handoff

## Estado medido (2026-11-11)

| cosa | número | comando |
|---|---|---|
| crates | 4 (`fm1-core`, `fm1-midi`, `fm1ctl`, `fm1-perf`) | `cargo metadata --no-deps` |
| app de UI | 1.261 líneas de Rust, binario `fast` de 6,1 MB | `wc -l crates/fm1-perf/src/*.rs crates/fm1-perf/src/panels/*.rs` |
| tests del dominio | **22**, en verde, sin aparato | `cargo test` |
| parámetros en el mapa | **239** (80 pista · 96 motor · 63 globales) | `python3 scripts/gen_nrpn.py` |
| clippy | limpio con `-D warnings` | `cargo clippy --all-targets -- -D warnings` |
| puerta completa | verde | `scripts/check.sh` |
| líneas de Rust | 3.060 (incluye la tabla generada) | `find crates -name '*.rs' \| xargs wc -l` |
| disco libre | 8,4 GB; `target/` 131 MB | `df -h /` |

Lo que **no** está hecho: persistencia de setlists en disco (M2) y reconciliación por el eco (M3). El set
de la app es el de ejemplo en `app.rs` (`demo_setlist`), todavía no sale de un archivo.

### La app en dos comandos

```bash
cargo run --profile fast -p fm1-perf                                  # la ventana
cargo run --profile fast -p fm1-perf -- --shot shots/panel.png --demo  # captura y sale
```

Teclas: `1–6` disparar · `←→` elegir franja · `↑↓` mover valor (`shift` de a 8) · `±` tempo (`⌥` de a 5) ·
`esc` pánico · `r` reconectar el sondeo. Las cuatro franjas son parámetros reales del mapa:
`FILTER` de la pista 1 (id 5, bipolar), `DJ FILT` (global 17, bipolar), reverb `SIZE` (8) y `MIX` (7).

## Verificación sin ojos

1. `scripts/check.sh` — fmt, clippy, tests, mapa al día y humo de bytes. Es la definición de terminado.
2. `fm1ctl ... --dry` — imprime los bytes y **no** abre puertos. Es cómo se verifica la codificación sin
   aparato; el humo del `check.sh` compara contra valores que ya se validaron a mano:
   `bpm 140` → `BF 63 01 / BF 62 00 / BF 06 40 / BF 26 00`, `section C` → `CF 02`, `project 3` → `CF 12`.
3. `fm1ctl ports` / `fm1ctl info` — sólo con aparato: puertos visibles e identidad por handshake
   (`FM-1_909` en JIANT 0.9.9-par12; stock V15 y V16 también contestan).

## Invariantes que no se rompen

- `fm1-core` no importa `midir` ni `eframe`. Si aparece, la capa está filtrada.
- `nrpn_table.rs` es salida de `scripts/gen_nrpn.py`. Editarlo a mano rompe `--check`.
- Los valores NRPN van **escalados** al rango del parámetro: `v = min + (x*(max-min) + 8191)/16383`.
  Mandar el número crudo (por ejemplo 140 para 140 BPM) hace que el aparato interprete otra cosa: es el
  error más fácil de cometer y el que ya apareció una vez acá.
- El canal importa: un NRPN de pista sólo lo escucha la pista que toca ese canal. Si no se mueve nada,
  lo primero a revisar es el canal y el ROUTE, no el mensaje.
- La app nunca bloquea el hilo de dibujo esperando al aparato: lectura en su hilo, envío best-effort.

## Qué NO hacer

- **No subir egui/eframe** sin ADR: 0.37 es una reescritura (`App::ui`, sin `TopBottomPanel`).
- **No agregar un webview** para la UI. La decisión está en ADR-0002 y es lo que mantiene el binario chico.
- **No sumar `incluir` un segundo árbol de specs**: `openspec/` es la fuente (ADR-0005).
- **No copiar `candil/src/app.rs` como forma**: 10.473 líneas en un archivo es exactamente lo que este
  proyecto evita.
- **No prometer ACK**: el aparato no confirma nada. Toda confirmación sale del eco (MIDI OUT ALL) o del
  editor protocol.
- **No tocar la flash del aparato desde acá.** Este repo manda MIDI; el flasheo vive en el proyecto
  `m-vave` (`probe/`, `firmware/`, `NOTAS.md`) con su propio procedimiento y su camino de vuelta.

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

## Deuda conocida

| causa | arreglo |
|---|---|
| La CLI paniquea con *broken pipe* si la salida se corta (`fm1ctl params \| head`): `println!` entra en pánico con EPIPE | escribir por `io::stdout()` ignorando `ErrorKind::BrokenPipe`; el `check.sh` lo esquiva con `grep` sin `-q` |
| Sin hot-plug: si el aparato se desconecta, `send` empieza a fallar y hay que reabrir | sondeo de `ports` cada N ms en la app (midir no notifica), y botón de reconectar |
| `Device::identify` (en el core) y `Port::identify` (en el adaptador) se solapan | dejar uno solo cuando la app de UI defina de quién es la responsabilidad |
| No hay prueba en vivo de los comandos de setlist | **cerrado 2026-11-11**: se enviaron `section B/A`, `project 2` y `bpm 140` con el aparato conectado; ver §Prueba en vivo |
| El listener no imprime tiempos: sirve para ver mensajes, no para medir tempo ni jitter | imprimir `Instant`/stamp por mensaje y calcular el intervalo entre pasos |

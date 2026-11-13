# AGENTS.md

Punto de entrada para cualquier agente (o persona) que tome `fm1lab`. Leer en este orden:

1. **`docs/HANDOFF.md`** — estado medido, invariantes, cómo verificar sin ojos.
2. **`docs/ROADMAP.md`** — qué sigue y cuándo se considera terminado.
3. **`docs/ARCHITECTURE.md`** — capas, regla de dependencia, qué está generado.
4. **`openspec/`** — la fuente de verdad de los cambios (ver `AGENTS.md` §Cambios).
5. Los ADR que toquen el área: `docs/adr/`.

## Comandos

```bash
cargo test                                   # 65 tests, sin aparato
cargo clippy --all-targets -- -D warnings    # cero warnings
scripts/check.sh                             # la puerta completa (fmt + clippy + tests + generado + humo)
python3 scripts/gen_nrpn.py                  # regenera el mapa NRPN desde docs/NRPN.md
python3 scripts/gen_nrpn.py --check          # falla si el mapa quedó viejo
cargo run --profile fast -p fm1ctl -- ports  # puertos MIDI visibles
cargo run --profile fast -p fm1ctl -- info   # identidad del aparato (handshake de lectura)
cargo run --profile fast -p fm1ctl -- clock  # fuente de reloj (int|usb|trs)
cargo run --profile fast -p fm1ctl -- projects # secciones con música del proyecto cargado
cargo run --profile fast -p fm1ctl -- song   # cadena de song
cargo run --profile fast -p fm1ctl -- mix    # pista y franjas que ve la app
cargo run --profile fast -p fm1ctl -- play 4 # arranca como reloj maestro 4 s y para
cargo run --profile fast -p fm1ctl -- setlist show.json --dry # valida un setlist y muestra los bytes
cargo run --profile fast -p fm1ctl -- bpm 140 --dry   # bytes, sin abrir puerto
cargo run --profile fast -p fm1ctl -- profile         # perfiles disponibles y el detectado
cargo run --profile fast -p fm1ctl -- profile x0x-1.2 # detalle de un perfil
cargo run --profile fast -p fm1ctl -- firmware        # biblioteca: identidad, sha y estado de paquetes
cargo run --profile fast -p fm1ctl -- firmware --install x0x-1.2.2 --dry  # valida sin escribir
cargo run --profile fast -p fm1ctl -- route jiant-v10.7.23   # la ruta segura, sin escribir
cargo run --profile fast -p fm1ctl -- flash x.fwsc --dry  # parsea un .fwsc; --yes es el único que escribe
cargo run --profile fast -p fm1-perf                  # la ventana de performance
cargo run --profile fast -p fm1-perf -- --read        # el set que armaría, sin ventana
cargo run --profile fast -p fm1-perf -- --mix 5       # sigue las franjas 5 s, sin ventana
cargo run --profile fast -p fm1-perf -- --setlist show.json # otro archivo de setlist
cargo run --profile fast -p fm1-perf -- --profiles    # abre el selector de perfil
cargo run --profile fast -p fm1-perf -- --library     # abre la biblioteca de firmwares
cargo run --profile fast -p fm1-perf -- --shot shots/panel.png --demo   # captura, sin ojos
cargo run --profile fast -p fm1-perf -- --shot shots/firmware.png --demo --library
cargo run --profile fast -p fm1-perf -- --shot shots/profiles.png --demo --profiles
cargo run --profile fast -p fm1-perf -- --shot shots/route-jiant.png --library --firmware jiant-v10.7.23
scripts/make_app.sh target/dist/fm1-perf target/FM1Lab.app  # bundle macOS
git tag vX.Y.Z && git push origin main --tags                # dispara .github/workflows/release.yml
openspec validate --specs --strict           # valida las specs
```

Perfiles: `fast` para iterar (~15 s por cambio), `release` para uso, `dist` para distribuir.

## Reglas duras

- **La regla de dependencia no se negocia.** `fm1-core` no depende de `fm1-midi`, ni de egui, ni de
  archivos: si necesita I/O, lo pide por un trait de `transport`. El adaptador vive afuera.
- **`crates/fm1-core/src/nrpn_table.rs` está generado.** No editarlo a mano: cambiar `docs/NRPN.md`
  (que viene del firmware) y correr `scripts/gen_nrpn.py`. `check.sh` falla si divergen.
- **La app no hardcodea un firmware: elige un perfil.** `fm1-core::profile` declara identidad,
  parámetros con acceso (`cc`/`nrpn`/`editor`), transporte y escenas; `generic`, `jiant-par12` y `x0x-1.2` son
  built-in y los del usuario van en `~/.config/fm1lab/profiles/`. `--profile` fuerza.
- **El flasheo es deliberado y vive en `fm1-flash`.** La biblioteca (`f`/`fm1ctl firmware`) verifica
  SHA-256, respalda config y paquetes en `~/.config/fm1lab/backups/` y pide confirmación; recién ahí
  `fm1-flash` habla OTA/Felucca. Nunca instalar en silencio y pausar transporte/sondeo/monitor antes.
  El instalador Python de `m-vave` queda como rescate.
- **La ruta la decide `fm1-core::route`, no quien instala.** Misma familia o desde el oficial, directo; de
  fork a fork, el oficial en el medio (la zona de datos de arriba no se reescribe y cada fork tiene su
  formato). Con el aparato en `ota-…` no se escribe: se termina esa actualización. `--straight` existe y
  avisa; no es un camino normal.
- **El tag manda la versión.** `release.yml` falla si `vX.Y.Z` no coincide con la versión del
  workspace; publica `FM1Lab.app` arm64/x86_64 y los alias estables que usa el cask. El cask es
  estático (`version :latest`), así que un release no requiere tocar el tap.
- **La biblioteca es dato, no código.** `firmwares.json` (raíz, embebido) lista los paquetes con su
  SHA-256; el usuario suma o pisa entradas en `~/.config/fm1lab/firmwares.json`. Los `.fwsc` se
  resuelven contra `FM1LAB_FIRMWARES` o `~/Proyectos/m-vave/firmware`; no clavar rutas en el código.
- **Dos protocolos, dos codificaciones.** El NRPN escala sobre el rango; el editor protocol (SysEx
  `F0 7D 46 4C …`) usa offset binary (`valor + 8192`). En par12 el firmware **no implementa NRPN**:
  `CLOCK`, proyectos, cadenas y los parámetros que toca la app van por el editor.
- **El reloj manda.** Con `CLOCK = INT` el firmware ignora `Start`/`Stop`/clock por USB. El play de la
  app pone `USB`, manda el clock y restaura el valor al parar; si algo queda colgado, `fm1ctl clock int`.
- **Ninguna dependencia nueva sin ADR.** Se justifica peso, licencia y mantenimiento. Criterio: este
  proyecto tiene que quedar liviano (binario de pocos MB, arranque instantáneo, sin webview).
- **egui/eframe está pinneado en 0.32 por ADR-0002.** Subir de versión es una reescritura, no un bump.
- **La UI no habla con el aparato.** Escribe intenciones en el dominio (`Device`) y el adaptador las
  manda. Si un panel necesita bytes, está en la capa equivocada.
- **El dibujo se verifica con `--shot`.** Un cambio de tema o de layout se cierra con una captura en
  `shots/`, no con "quedó lindo": es el equivalente visual de `cargo test`.
- **La app no crece en un archivo.** `app.rs` de la app de UI se queda bajo ~1.500 líneas, con los
  paneles en `panels/*.rs`. Es la lección del proyecto hermano (`candil`), donde `app.rs` llegó a 10.473
  líneas y ya no se puede probar sin pantalla.
- **JSON atómico** (temporal + `rename`) para todo lo que se persista. nunca `fs::write` directo.
- **Disco.** `target/` es lo que más crece: `cargo clean --profile dist` antes de builds grandes y no
  reactivar `incremental` en `dev`/`fast`. Un link que falla con `errno=28` es disco, no código.
- **El aparato no avisa nada.** No hay ACK de los mensajes: la UI es optimista y reconcilia con el eco
  (MIDI OUT ALL) o con el editor protocol. No inventar confirmaciones.

## Cambios

`openspec/` es la fuente de verdad: un cambio se propone, se aprueba y se archiva ahí; `docs/` describe
el estado y `docs/adr/` las decisiones ya cocinadas. No crear un segundo árbol de specs en paralelo
(regla de `agent-handoff-package`).

## Skills del repo

`.agents/skills/` tiene las skills que usan los agentes en este repo: `clean-architecture` y
`kent-beck-style` (MIT, de nathankim0) e `impeccable` (Apache-2.0, de pbakaus). Se vendorizaron para que
viajen con el código. Ojo con `impeccable`: su detector, su modo `live` y sus reglas HTML son **sólo
web** (lo dice su `reference/routing.md`); acá se usa su vocabulario y su rúbrica de `critique`/`audit`,
aplicadas a mano a una UI de egui.

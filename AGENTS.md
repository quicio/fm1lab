# AGENTS.md

Punto de entrada para cualquier agente (o persona) que tome `fm1lab`. Leer en este orden:

1. **`docs/HANDOFF.md`** — estado medido, invariantes, cómo verificar sin ojos.
2. **`docs/ROADMAP.md`** — qué sigue y cuándo se considera terminado.
3. **`docs/ARCHITECTURE.md`** — capas, regla de dependencia, qué está generado.
4. **`openspec/`** — la fuente de verdad de los cambios (ver `AGENTS.md` §Cambios).
5. Los ADR que toquen el área: `docs/adr/`.

## Comandos

```bash
cargo test                                   # 22 tests del dominio, sin aparato
cargo clippy --all-targets -- -D warnings    # cero warnings
scripts/check.sh                             # la puerta completa (fmt + clippy + tests + generado + humo)
python3 scripts/gen_nrpn.py                  # regenera el mapa NRPN desde docs/NRPN.md
python3 scripts/gen_nrpn.py --check          # falla si el mapa quedó viejo
cargo run --profile fast -p fm1ctl -- ports  # puertos MIDI visibles
cargo run --profile fast -p fm1ctl -- info   # identidad del aparato (handshake de lectura)
cargo run --profile fast -p fm1ctl -- bpm 140 --dry   # bytes, sin abrir puerto
cargo run --profile fast -p fm1-perf                  # la ventana de performance
cargo run --profile fast -p fm1-perf -- --shot shots/panel.png --demo   # captura, sin ojos
openspec validate --specs --strict           # valida las specs
```

Perfiles: `fast` para iterar (~15 s por cambio), `release` para uso, `dist` para distribuir.

## Reglas duras

- **La regla de dependencia no se negocia.** `fm1-core` no depende de `fm1-midi`, ni de egui, ni de
  archivos: si necesita I/O, lo pide por un trait de `transport`. El adaptador vive afuera.
- **`crates/fm1-core/src/nrpn_table.rs` está generado.** No editarlo a mano: cambiar `docs/NRPN.md`
  (que viene del firmware) y correr `scripts/gen_nrpn.py`. `check.sh` falla si divergen.
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

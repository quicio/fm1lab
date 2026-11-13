## 1. Lectura y sondeo

- [x] 1.1 `Mix` y `read_mix` en `link.rs`: `TRACK` + `GET` de FILTER/DJ FILT/SIZE/MIX.
- [x] 1.2 `Probe` publica `Mix` cada ~600 ms con descubrimiento cada 1,5 s; `take_mix`.
- [x] 1.3 La app aplica el snapshot a las franjas y al rótulo `cabezal tN`, con guarda de 400 ms.

## 2. CLI

- [x] 2.1 `fm1ctl mix` imprime pista y valores.

## 3. Verificación y docs

- [x] 3.1 `cargo test` y `scripts/check.sh` verde.
- [x] 3.2 Cambiar un valor por el editor y verlo en `fm1ctl mix`; girar una perilla y verlo en la
      ventana.
- [x] 3.3 `HANDOFF.md`, `ARCHITECTURE.md`, `ROADMAP.md` (M3 cerrado) y `--shot`.
- [x] 3.4 `openspec validate --specs --strict`.

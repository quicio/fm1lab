---
id: architecture
status: vigente
version: 0.1.0
updated: 2026-11-11
---

# Arquitectura

## Capas y regla de dependencia

```
        apps (entrega)                 fm1ctl · fm1-perf (egui)
             │  usan
             ▼
     adaptador (infraestructura)       fm1-midi (midir / CoreMIDI)
             │  cumple los traits
             ▼
   dominio (reglas, sin I/O)           fm1-core: nrpn · message · device · setlist
```

Las flechas del código apuntan hacia adentro. `fm1-core` declara `MidiOut` y `MidiIn` en
`transport.rs`; `fm1-midi` los implementa con `midir`. Ninguna regla del dominio conoce CoreMIDI, un
nombre de puerto ni un archivo.

Lo que esto compra, concretamente: los 22 tests del dominio corren sin aparato y sin pantalla, y el
`--dry` de la CLI verifica la codificación de bytes en un segundo.

## Módulos

| módulo | hace | no hace |
|---|---|---|
| `fm1-core/src/nrpn.rs` | espacios de parámetros (pista/global), búsqueda por nombre o id, escalado a 14 bits, E1..E8 por motor | no abre puertos ni sabe de canales MIDI reales |
| `fm1-core/src/nrpn_table.rs` | **generado** desde `docs/NRPN.md` (239 parámetros: 80 de pista, 96 de motor, 63 globales) | no se edita a mano |
| `fm1-core/src/message.rs` | bytes: CC, PC, notas, NRPN de 4 mensajes, handshake de identidad | no tiene estado ni decide |
| `fm1-core/src/device.rs` | intenciones: `set_bpm`, `set_global`, `set_track`, `goto_section`, `load_project`, `load_sound`, `play(item)`, `panic` | no sabe si abajo hay USB, TRS o un grabador de prueba |
| `fm1-core/src/setlist.rs` | modelo serializable del set (sección, proyecto, sonido) con validación de rangos | no persiste: eso es del adaptador |
| `fm1-core/src/transport.rs` | los traits `MidiOut`/`MidiIn` y los dobles `Recorder`, `ScriptedIn`, `Silent` | no transporta nada |
| `fm1-midi/src/lib.rs` | descubre puertos por nombre (`fm1`, `jiant`, `felucca`, `smk`), abre salida, lee en un hilo, `identify()` | no decide qué mandar |
| `fm1ctl/src/main.rs` | CLI: puertos, identidad, mensajes crudos, intenciones, setlists, `--dry` | no dibuja |

## El protocolo, en una tabla

| intención | mensaje | canal |
|---|---|---|
| tempo, swing, DJ filter, reverb | NRPN 14 bits: CC99=1, CC98=id, CC6/CC38 | 16 |
| parámetro de una pista | NRPN: CC99=0, CC98=id | el de la pista (T1..T6 = 1..6) |
| level, pan, cutoff, res, sends | CC 7, 10, 74, 71, 91/93/94 | el de la pista |
| los ocho EDIT del motor | CC 16..23 | el de la pista |
| sección A–D | Program Change 0–3 | 16 |
| proyecto 1–6 | Program Change 16–21 | 16 |
| sonido de la lista PRESETS | Program Change n | el de la pista |
| identidad del aparato | SysEx `F0 00 32 45 00 00 00 40 7F F7` (lectura) | — |

Fuente: manual del firmware (§14) y `docs/NRPN.md`, que el propio firmware genera desde `params.c` con
un test que falla si divergen. La fórmula del valor es
`v = min + (x * (max - min) + 8191) / 16383`.

## Lo que está generado, y por qué

`docs/NRPN.md` → `crates/fm1-core/src/nrpn_table.rs`, por `scripts/gen_nrpn.py`. Es el mismo trato que
hace el firmware con su documentación: la tabla no se escribe a mano para que no pueda mentir. Un
`--check` en la puerta de CI detecta el desfase.

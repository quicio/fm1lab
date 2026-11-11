---
id: adr-0004
title: El mapa NRPN se genera desde la documentación del firmware
status: aprobada
date: 2026-11-11
supersedes: —
---

# 0004 — Mapa NRPN generado

## Contexto

El firmware publica su mapa de parámetros en `docs/NRPN.md`, generado a su vez desde `params.c` con un
test que falla si la documentación y el firmware divergen. Ese archivo tiene 239 entradas con id, nombre,
rango y valor por defecto, y cambia con cada versión del firmware.

Escribir esas 239 filas a mano en Rust garantiza que en dos versiones estén mal.

## Decisión

`docs/NRPN.md` es la fuente. `scripts/gen_nrpn.py` lo parsea y emite
`crates/fm1-core/src/nrpn_table.rs`, con cabecera `@generated` y `#[rustfmt::skip]`. La puerta corre
`gen_nrpn.py --check` y falla si el `.rs` no coincide con el `.md`.

## Consecuencias

- La tabla no puede mentir: si el firmware cambia, se actualiza el `.md` y se regenera.
- Los rangos vienen con la fuente, así que la validación de valores (y el escalado a 14 bits) es real y no
  una lista de casos.
- El generador tiene que fallar visiblemente si el formato del `.md` cambia (hoy: si parsea menos filas
  de las esperadas, los tests de `nrpn` lo delatan).
- Editar `nrpn_table.rs` a mano es un error de proceso: se revierte al regenerar.

## Alternativas descartadas

- **Tabla a mano.** Divergiría en silencio, que es el peor modo de falla para un protocolo.
- **Leer el `.md` en tiempo de ejecución.** Mete I/O y un parser en el dominio (ADR-0001 lo prohíbe) y
  rompe el binario autocontenido.

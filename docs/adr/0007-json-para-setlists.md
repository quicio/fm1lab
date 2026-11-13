---
id: adr-0007
title: JSON para persistir setlists
status: aprobada
date: 2026-11-12
supersedes: —
---

# 0007 — `serde_json` para el setlist

## Contexto

`fm1-core` ya deriva `serde` para el modelo del setlist, pero `serde_json` sólo estaba como
dependencia de test. M2 pide guardar y cargar el show en disco, y la app y la CLI tienen que compartir
el mismo formato. La regla del repo pide justificar peso, licencia y mantenimiento antes de sumar una
dependencia de runtime.

## Decisión

`serde_json 1` en el crate nuevo `fm1-store`, formato JSON con versión. No es una dependencia nueva
del proyecto: ya estaba en `Cargo.lock` por los tests de `fm1-core` y se promueve a runtime. El
dominio sigue sin saber de archivos.

## Consecuencias

- Archivo legible y difftable; se puede editar a mano y validar al cargar.
- Misma versión y cadena de dependencias que ya se compilaba para tests: el peso marginal es cero.
- La escritura atómica y el backup de corruptos son nuestros (`fm1-store`), no del formato.
- El esquema queda versionado a mano (`version: 1`); una versión futura se rechaza hasta migrarla.

## Alternativas descartadas

- **TOML.** Suma el crate `toml` y no aporta nada para una lista de pasos.
- **`bincode` u otro binario.** Archivo opaco, no editable ni revisable en un diff.
- **Escribir JSON a mano.** Parser propio sin necesidad; el error más caro es el que no se ve.

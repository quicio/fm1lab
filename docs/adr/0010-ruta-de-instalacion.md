---
id: adr-0010
title: La ruta de instalación pasa por el oficial cuando cambia la familia
status: aprobada
date: 2026-11-13
supersedes: —
---

# 0010 — Ruta de instalación con el oficial en el medio

## Contexto

La instalación reemplaza la zona de aplicación de la flash (`0x4000`–`0x92FFF`) pero **no** la zona de datos
de arriba (presets y proyectos, alrededor de `0x97000` y `0xFC000`). Cada fork define ese formato por su
cuenta. Escribir un fork encima de otro deja los datos del anterior y el firmware nuevo los lee mal: es el
soft-brick documentado de Felucca 0.9 → 1.0.x, que sólo se arreglaba borrando dos regiones con
`jl-uboot-tool`.

El instalador (ADR-0009) sabe escribir; lo que no sabía era **decidir**. La decisión estaba en la cabeza de
quien lo usaba, y en un archivo aparte del proyecto (`m-vave/tools/fmroute.py`) que era Python.

## Decisión

La decisión es dominio puro en `fm1-core::route` y el aparato no se toca sin pasar por ella:

| desde | a | ruta |
|---|---|---|
| oficial de fábrica | cualquiera | directo |
| misma familia | actualización | directo |
| un fork | otro fork | **oficial en el medio** |
| modo update (`ota-…`) | — | se niega: se termina esa escritura |

- `Family` sale del catálogo (para el paquete) o de la identidad del handshake (para el aparato), nunca de
  adivinar: lo que no está en la tabla es `Family::Other`, y un fork desconocido **también** pasa por el
  oficial.
- `plan()` devuelve pasos con su motivo y se niega con un `Refusal` explícito (`MidUpdate`, `NoStock`,
  `NeedsBackup`, `AlreadyThere`). No hay un camino que escriba «porque sí».
- `fm1ctl` aplica la ruta en `flash` y en `firmware --install`, y ofrece `route` para verla sin escribir.
  `--straight` existe como escape explícito y avisa en pantalla lo que puede pasar.
- Después de cada paso se compara la identidad que reporta el aparato con la esperada: si no coincide, no se
  sigue con el paso siguiente.

## Consecuencias

- Cambiar de fork ya no puede semi-brickear el aparato por olvido: la herramienta no ofrece ese camino.
- La ruta se puede ensayar sin hardware (`route --from IDENT`) y se testea sin aparato: 9 casos en
  `fm1-core::route`, incluida la identidad corta de X0X (`FM-1_901`), que si se lee mal hace que un aparato
  corriendo X0X parezca otra cosa.
- Pasar por el oficial resetea la zona de datos: la CLI lo dice antes de escribir y el backup del lab
  (`~/.config/fm1lab/backups/`) corre igual, aunque no puede exportar proyectos de un firmware ajeno.
- El instalador Python de `m-vave` queda sólo como rescate, no como camino normal.

## Alternativas descartadas

- **Dejarlo en el instalador.** El instalador escribe lo que le dan; si decide, hay dos lugares que deciden.
- **Una regla en la documentación.** Es exactamente lo que falló: la regla existía y se podía olvidar.
- **Prohibir todo cambio de familia.** Demasiado: pasar por el oficial es barato y funciona.

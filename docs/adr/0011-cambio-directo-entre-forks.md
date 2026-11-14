---
id: adr-0011
title: Los cambios entre forks son directos; el stock se instala con M-UPGRADE
status: aprobada
date: 2026-11-14
supersedes: 0010 (la ruta por el oficial no es ejecutable con el OTA Felucca)
---

# 0011 — Cambio directo entre forks y stock por M-UPGRADE

## Contexto

ADR-0010 mandaba pasar por el firmware oficial cuando se cambiaba de fork, para que la zona de datos
(presets/proyectos, `0x97000`/`0xFC000`) quedara en un formato conocido. En la práctica ese paso **no
se puede ejecutar**: `fm1-stock-official.fwsc` no es un paquete Felucca (no contiene
`FELUCCA-LOADER-1`) y el firmware que corre corta la lectura del paquete a los 4 pedidos. El stock sólo
se instala con **M-UPGRADE**, no por el protocolo OTA que usa `fm1-flash`.

Al mismo tiempo se verificó el cambio directo: X0X 1.2.2 → SLOOP 2.8.5 con una sola escritura. El
aparato reinició y reportó `FM-1_900`. En el primer intento la verificación dio timeout a los 40 s,
pero el aparato volvió después: el timeout era corto, no la escritura.

## Decisión

- **Misma familia o desde el oficial:** directo.
- **Fork → fork:** directo, en **una sola escritura**, con un aviso explícito de que la zona de datos
  puede quedar incompatible. El backup de config corre igual; los proyectos del firmware anterior no
  se pueden exportar desde acá.
- **Destino stock:** se rechaza con un mensaje que manda a M-UPGRADE. `--force` no lo hace instalable.
- **Modo update (`ota-…`):** no se empieza otra escritura; se termina esa.
- Los timeouts de loader y reinicio suben a **120 s** cada uno: el aparato tarda más que el instalador
  original en enumerar el loader y en volver.

`fm1-core::route` sigue siendo la única capa que decide; `fm1ctl route` lo muestra sin escribir y la
biblioteca aplica lo mismo con un clic.

## Consecuencias

- Cambiar de firmware ya funciona sin un paso intermedio imposible.
- La zona de datos puede quedar con formato viejo; si un fork no arranca, el rescate es M-UPGRADE o el
  instalador Python de `m-vave`. El aviso aparece antes de confirmar.
- El stock deja de ser una opción de la app; es una operación de recuperación con la herramienta
  oficial.
- La verificación ya no reporta un falso error por timeout en aparatos lentos.

## Alternativas descartadas

- **Mantener el paso por el oficial.** Falla siempre con el stock V16 actual: el usuario no puede
  cambiar de firmware.
- **Implementar M-UPGRADE en `fm1-flash`.** Es otro protocolo/formato de paquete; el instalador oficial
  queda como camino de recuperación.
- **Escribir stock directo con `--force`.** El aparato corta la lectura; no es un problema del host.

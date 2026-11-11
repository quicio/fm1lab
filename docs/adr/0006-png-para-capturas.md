---
id: adr-0006
title: png para la captura headless de la app
status: aprobada
date: 2026-11-11
supersedes: —
---

# 0006 — `png` para las capturas

## Contexto

La regla del repo es que ninguna dependencia entra sin justificar peso, licencia y mantenimiento. `--shot`
tiene que guardar una captura de la ventana y salir: es la única forma de verificar el dibujo sin ojos
(el patrón viene de `candil`, y acá es más necesario porque la UI se dibuja con `Painter`, no con widgets
que se puedan inspeccionar por nombre).

`eframe` entrega la captura como `egui::ColorImage` (pixeles RGBA en memoria). Falta el encoder.

## Decisión

`png 0.17` (MIT/Apache-2.0), sólo en `fm1-perf`. Cero dependencias transitivas, ~1,2 s de compilación.

## Consecuencias

- `fm1-perf --shot shots/panel.png` produce un PNG RGBA y cierra: la puerta visual deja evidencia en disco.
- `--shot --demo` siembra el historial con envíos de ejemplo, para que la captura muestre el panel en un
  estado plausible sin mandar nada al aparato.
- No se agregó `image` (la navaja suiza, con decenas de códecs): acá sólo se escribe un formato.
- La captura no corre en la puerta automática: abrir una ventana de GL necesita sesión gráfica. `check.sh`
  verifica bytes y dominio; la captura es manual y bajo demanda.

## Alternativas descartadas

- **`image`.** Diez veces el peso para escribir un PNG.
- **Encoder propio.** Un zlib a mano para ahorrar un crate no se paga.
- **Capturar con `screencapture` de macOS.** Depende del permiso de grabación de pantalla y del ID de
  ventana: frágil y no verificable en otra máquina.

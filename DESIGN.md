# DESIGN

Sistema visual de la app. Se completa con valores reales cuando exista la UI (M1); lo que sigue es lo ya
decidido y lo que no se negocia.

## Principios

- **Escenario primero.** Se lee de pie, con poca luz, a un metro de la pantalla: alto contraste, tipografía
  grande en lo que se mira, cero decoración que no informe.
- **Teclado antes que mouse.** Todo lo que se dispara en vivo tiene una tecla, y se muestra al lado.
- **Una sola pantalla.** Sin pestañas, sin asistentes: lista de set, estado del aparato, cuatro controles.
- **El estado se ve, no se adivina.** Conectado / sin aparato / enviando se distingue de un vistazo.

## Herramientas

- egui/eframe 0.32 sin webview (ADR-0002). No hay CSS, ni DOM, ni motor de layout.
- `impeccable` está vendorizada en `.agents/skills/` **como vocabulario y rúbrica** (`critique`, `polish`,
  `typeset`, `distill`, `quieter`). Su detector, su modo `live` y sus reglas HTML son sólo web y no corren
  sobre egui; aplicarlas es trabajo del agente, a mano, sobre el código.

## Tokens

| grupo | decisión |
|---|---|
| paletas | cinco presets como en `candil` (Papel, Noche, Claro, Ámbar, Contraste), reusando `theme.rs` |
| radios | `SM`/`MD`/`LG` del sistema de `candil` (`theme::radius`) |
| espacios | escala de `theme::space` (4/8/12/16/24) |
| tipografía | tamaños de `theme::text` (`BODY`, `TITLE`, …) más un tamaño "escenario" para lo que se lee en vivo |
| estados | `accent` para lo activo, `error` para lo que falta (aparato desconectado), sin colores nuevos |

La tabla de valores concretos se copia de `candil/src/theme.rs` en M1, no se inventa acá.

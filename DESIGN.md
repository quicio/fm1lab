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

Tema primario **Ceniza** (oscuro, de escenario). Único acento: verde señal. Nada más tiene color: si algo
no está activo o no es una advertencia, es gris o pelo de línea.

| token | valor | uso |
|---|---|---|
| `ink` | `#07080A` | fondo de la ventana |
| `panel` | `#0B0D10` | superficie de paneles |
| `raise` | `#101317` | fila o control levantado (casi invisible) |
| `line` | `rgba(233,237,242,.09)` | pelo de 1 px: separadores |
| `line-2` | `rgba(233,237,242,.18)` | pelo de 1 px cuando marca estado |
| `text` | `#E9EDF2` | texto principal |
| `dim` | `#7D8691` | etiquetas y valores secundarios |
| `dimmer` | `#4D545D` | números de paso, ayudas de teclado |
| `signal` | `#C8F751` | **lo vivo**: paso actual, transporte, conexión, valor modificado |
| `hot` | `#FF6B4A` | error o pánico; nunca decorativo |

Reglas de composición:

- **Pelo de 1 px, no cajas.** No hay sombras, no hay tarjetas anidadas, no hay esquinas redondeadas más
  allá de 4 px. La jerarquía se hace con espacio y con líneas.
- **Escala visible.** Una regla de marcas (ticks de 12 px y 60 px) arriba del cuerpo: es la única
  decoración permitida, y es información de escala, no ornamento.
- **Un solo acento.** `signal` aparece sólo en lo que está pasando ahora. Si todo brilla, nada brilla.
- **Números tabulares** siempre (`font-variant-numeric: tabular-nums`), en monoespaciada, para que un
  valor que cambia no mueva el layout.
- **Cifra grande de estado.** La sección actual se dibuja con un glifo de 104 px y peso 200: es lo que se
  lee desde el escenario, sin acercarse.

| grupo | decisión |
|---|---|
| paletas | **Ceniza** (primaria). Variantes futuras: `Ceniza · ámbar` (acento `#FFB020`) y `Papel` (claro) |
| radios | `SM 2`, `MD 4`, `LG 8` px (más chico que `candil`: acá no hay tarjetas) |
| espacios | 4 / 8 / 12 / 16 / 24 / 34 px |
| tipografía | sans del sistema para texto; mono para cifras; etiquetas en mayúsculas con `letter-spacing .16em` y 9,5–10 px |
| estados | `signal` = activo/enviado/escuchando · `hot` = falla · `dim` = inactivo · nunca un color nuevo |

El mock ejecutable de este tema es `design/mock-panel.html` (referencia visual, no se compila). Al portar a
egui, los valores de arriba van a `theme.rs` del crate de UI; los nombres de token no cambian.


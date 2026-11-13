## Context

Ver `proposal.md` y ADR-0008. Hoy el protocolo de JIANT está en el camino crítico: `fm1-core::device`,
`fm1-core::editor`, `fm1-midi::ClockMaster` y el setlist. Los otros firmwares (X0X, TRIBE, SLOOP,
stock) exponen sobre todo CC/PC/NRPN y no tienen el editor de JIANT. El lab tiene que servir a todos
sin perder lo que JIANT ya hace.

## Goals / Non-Goals

**Goals:**

- Un modelo de **perfil** que describa el firmware y sea la única fuente de la UI.
- `generic` útil de verdad: CC estándar por pista, sin conocer el firmware.
- JIANT como perfil, con paridad funcional (secciones, editor, reloj).
- Franjas asignables y persistidas por perfil.
- Aprender un CC/NRPN en vivo y guardarlo.

**Non-Goals:**

- Flashear firmware (sigue en `m-vave`).
- Editar sonidos/presets como un editor completo.
- Soportar SysEx propietarios de cada firmware en la fase 1; sólo JIANT (editor) y CC/NRPN.
- Igualar todas las funciones de cada firmware; el lab expone parámetros y acciones declaradas.

## Decisions

### El perfil es data, no código

- JSON con `id`, `name`, `match`, `params[]`, `transport`, `scenes`, `capabilities`.
- `params[]`: `{ id, label, scope, min, max, default, access }`; `access` es
  `{ kind: "cc" | "nrpn" | "editor", ... }`.
- Built-in `generic` (CC estándar de 6 pistas) y `jiant-par12` (editor; se genera desde la tabla
  actual para no escribir 239 entradas a mano).
- Perfiles de usuario en `~/.config/fm1lab/profiles/*.json`; el `id` de usuario gana sobre el built-in.
- Alternativa descartada: un trait Rust por firmware. Agregar un firmware de CC/NRPN no debería
  requerir recompilar.

### Acceso: una capa, tres protocolos

- `fm1-midi` suma `read_param`/`write_param`/`watch` que miran `ParamAccess`:
  - `editor`: el actual `Port::editor` (GET/SET, offset binary).
  - `cc`: `CC ch cc value`; la lectura no existe salvo que el firmware la mande (monitor/learn).
  - `nrpn`: CC99/98 + CC6/38; igual, escritura sin lectura en par12.
- El `editor` de JIANT queda como un adaptador; `gm1-core::editor` no desaparece, se usa desde el
  perfil.
- El `watch` sólo incluye parámetros `editor` (legibles); los CC/NRPN se actualizan por monitor o por
  el último valor escrito.

### Detección y caída a genérico

- Identidad OTA (`FM-1_909`, etc.) + INFO del editor (cuando contesta).
- `match`: lista de subcadenas/regex simples por identidad y versión; el primero que matchea.
- Si nada matchea o el aparato no contesta: `generic`.
- `--profile <id>` fuerza; `--profile generic` sirve para explorar un firmware desconocido.
- El perfil activo se muestra en la cabecera; `p` o un selector lo cambia.

### UI por capacidades

- `transport`: `clock_master` (JIANT: setea `CLOCK` y manda el clock), `midi` (Start/Stop/clock
  genéricos) o `none`.
- `scenes`: `program_change` (JIANT: secciones 0–3 / proyectos 16–23), `patterns`, `clips` o `none`.
- `setlist` y el bloque de transporte se dibujan sólo si hay capacidad; el resto del panel (link,
  franjas, monitor) siempre está.
- Las franjas vienen del `panel.json` del perfil; si el perfil no tiene un parámetro guardado, se usa
  el default del perfil.

### Monitor/learn

- Hilo de entrada dedicado (o el mismo del `Probe`) que junta los últimos N mensajes; el dibujo lee un
  snapshot.
- Aprender: captura el próximo CC/NRPN, propone `{kind, channel/msb, cc/lsb, value range}` y, al
  confirmar, agrega el parámetro al perfil de usuario (JSON atómico).
- Para CC/NRPN no hay lectura: el valor mostrado es el último enviado o el capturado; el learn es la
  forma de poblarlo.

### Fases

1. Perfil + `generic` + `jiant-par12` + acceso + detección + franjas asignables + UI por capacidades.
2. Monitor + learn + perfiles de usuario.
3. Escenas genéricas (patterns/clips/acciones PC/CC/nota) y más perfiles.

## Risks / Trade-offs

- **CC/NRPN sin lectura** → las franjas CC no siguen una perilla del aparato hasta que el firmware la
  mande; el monitor/learn cubre el caso y el perfil declara si es legible.
- **Identidades compartidas** (par12/gen2 dicen `FM-1_909`) → se desempata con INFO/versión; si no,
  el usuario elige el perfil.
- **Perfiles de usuario inválidos** → se avisa y se usa el built-in; nunca se pisa el archivo.
- **Deriva del firmware** → el perfil es data; actualizarlo no toca el código.
- **El setlist JIANT no aplica a otros firmwares** → se oculta por capacidad; no se inventa una
  sección que el firmware no tiene.
- **Más indirección** → tests del modelo/acceso y el perfil genérico como red.

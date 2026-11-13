## Context

Ver `proposal.md`. `Setlist` ya deriva `serde` en `fm1-core` (datos puros, sin I/O); `serde_json` sólo
está como dependencia de test. La app hoy arma el set leyendo el aparato (`device-read`) y `fm1ctl`
sólo valida por `--dry`, no tiene un archivo que abrir.

## Goals / Non-Goals

**Goals:**

- Un archivo versionado, legible y atómico, compartido por la app y la CLI.
- La app elige entre archivo y aparato sin que una lectura pise un set guardado.
- Verificar la ida y vuelta sin abrir la ventana ni el puerto.

**Non-Goals:**

- Editar el set desde la UI (todavía se edita el JSON a mano o se guarda el set del aparato).
- Sincronizar entre máquinas o resolver conflictos de escritura concurrente.
- Leer/escribir proyectos o sonidos del aparato desde el archivo.

## Decisions

### Crate `fm1-store` como adaptador de disco

- `fm1-core` no puede tocar archivos (regla de dependencia). La app y la CLI necesitan el mismo
  formato, así que va en un crate aparte que depende de `fm1-core` y de `serde_json`.
- Alternativa descartada: duplicar el store en `fm1-perf` y `fm1ctl` (dos formatos que se despegan) o
  meterlo en `fm1-midi` (no tiene nada que ver con MIDI).

### Formato `{ "version": 1, "name": "...", "items": [...] }`

- El `Item` conserva su `tag`/`content` actual (`section`, `project`, `sound`); el wrapper agrega la
  versión. Se deserializa con `#[serde(flatten)]` para no reescribir el modelo.
- Versión desconocida = error: un archivo de una versión futura no se interpreta a ciegas.
- Al cargar se corre `Setlist::validate()`: rangos de sección, proyecto y sonido.

### Escritura atómica de verdad

- Se escribe `destino.tmp` en el **mismo directorio** (el `rename` tiene que ser atómico), se hace
  `sync_all()` y se renombra sobre el destino. Nunca `fs::write` directo.
- Si el destino existe y el `rename` falla, el temporal se borra y el original queda intacto.
- El `rename` que reemplaza es de Unix; el proyecto es macOS (CoreMIDI). Se documenta.

### Corrupto: `.bak`, no fábrica silenciosa

- JSON inválido o paso fuera de rango → se mueve el archivo a `.bak` (pisando el `.bak` anterior) y se
  devuelve el error. La app lo muestra; no se inventa un set que el usuario no guardó.
- Archivo ausente ≠ corrupto: se devuelve `NotFound` y la app usa el set del aparato. La plantilla se
  escribe recién con `setlist init` o con `s` en la app.

### Ruta por defecto y override

- `FM1LAB_SETLIST` si está; si no, `$HOME/.config/fm1lab/setlist.json`. Se crea el directorio al
  guardar. Sin el crate `dirs` (no hace falta para una ruta).
- `fm1-perf --setlist <archivo>` y `fm1ctl setlist <archivo>` permiten otro archivo sin tocar el
  default.

### La app: origen del set

- `Origin::{File, Device}`. Al arrancar, si el archivo existe y valida → `File`; si no → `Device`.
- Una lectura automática (sondeo) sólo reemplaza el set si el origen es `Device`; los valores de las
  franjas y la pista seleccionada se actualizan siempre.
- `r` pasa a `Device` y pide una lectura; `l` carga el archivo y pasa a `File`; `s` guarda el set
  actual en el archivo.
- El tempo sigue con el candado de `device-read`: se adopta una vez y después manda la app.

### CLI para verificar sin ojos

- `fm1ctl setlist <archivo>` valida, imprime nombre y pasos. Con `--dry`, manda cada paso a
  `Device<Capture>` e imprime los bytes (PC de sección/proyecto/sonido), sin abrir puerto.
- `fm1ctl setlist init <archivo>` escribe la plantilla A–D.
- El humo de `check.sh` crea un temporal, inicializa, lo inspecciona con `--dry` y lo borra.

## Risks / Trade-offs

- **JSON editable a mano** → la validación atrapa rangos, no intención; un paso válido puede apuntar a
  un proyecto que el aparato no tiene y el firmware lo ignora (sin ACK).
- **`rename` atómico es de Unix** → el proyecto es macOS; si algún día va a Windows, hay que revisar.
- **Dos instancias escribiendo** → gana la última; no hay lock. Fuera de alcance.
- **Sin `$HOME`** → error claro; `FM1LAB_SETLIST` permite fijar la ruta.
- **El set guardado no se reconcilia con el aparato** → `r` es el camino para traer el set real;
  reconciliar automáticamente es M3.

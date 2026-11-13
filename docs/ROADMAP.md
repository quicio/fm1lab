---
id: roadmap
status: vigente
version: 0.1.0
updated: 2026-11-11
---

# Roadmap

Cada milestone se cierra con un comando, no con una opinión. Los cambios concretos se proponen en
`openspec/`; acá está el orden y el criterio de salida.

## M0 — Dominio y CLI (cerrado)

Torre de tres capas, mapa NRPN generado, `fm1ctl` con `--dry`, puerta verde.

**Salida:** `scripts/check.sh` verde y `fm1ctl bpm 140 --dry` imprimiendo los cuatro mensajes.

## M1 — App de performance mínima (cerrado el 2026-11-11, con deuda)

La ventana existe y dispara: setlist, cifra de sección, «siguiente», historial de envíos y cuatro
parámetros reales del mapa. Tema «Ceniza» portado a `theme.rs`; verificación por captura
(`shots/panel.png`, con `design/mock-panel.html` como referencia visual).

**Salida (cumplida):** `fm1-perf --shot shots/panel.png --demo` genera la captura sin intervención, y con
el aparato conectado `1–6` dispara secciones y proyectos.

**Corregido el 2026-11-12:** `1–6` no arrancaba nada porque la JIANT ignora el `Start` con
`CLOCK = INT`, y la setlist de ejemplo apuntaba a proyectos vacíos. La app ahora manda el reloj y arma
el set leyendo el aparato; ver M3.

**Deuda que pasó a M2 (cerrada el 2026-11-12):** el set no se guardaba; ahora va a
`~/.config/fm1lab/setlist.json`. El estado «sin aparato» sigue siendo un punto de color, sin
deshabilitar lo que no puede funcionar.

## M2 — Persistencia y setlists reales (cerrado el 2026-11-12)

`fm1-store`: JSON versionado, escritura atómica (temporal + `rename`), validación de rangos al cargar,
copia `.bak` si el archivo está corrupto y plantilla A–D. La app carga el archivo al arrancar y lo
guarda con `s`; `r` trae el set del aparato. `fm1ctl setlist` valida y muestra el archivo, con `--dry`
imprime los bytes. ADR-0007 justifica `serde_json` en runtime.

**Salida (cumplida):** test de ida y vuelta de 12 pasos en `cargo test`; `scripts/check.sh` incluye el
humo `setlist init` → `setlist --dry`; `fm1-perf --setlist <archivo>` carga el set sin tocar el
default.

## M3 — Seguir el aparato (lectura) — cerrado el 2026-11-12

**Hecho:** editor protocol en `fm1-core` (`editor.rs`), lectura de secciones/cadena/BPM por el hilo de
sondeo, y `ClockMaster` (la app manda `Start`/`Stop`/clock y restaura `CLOCK`). El set y el tempo
salen de lo que el aparato tiene cargado. El tempo y las cuatro franjas se escriben por el editor: el
firmware 0.9.9 no implementa NRPN, así que el camino NRPN no movía nada. M2 agregó el setlist en
archivo, que manda sobre el sondeo hasta que se aprieta `r`.

**Seguimiento:** el firmware tampoco manda eco de NRPN (su MIDI OUT sólo emite notas y clock). La app
sigue las franjas consultando `TRACK` + cuatro `GET` cada ~500 ms en el hilo de sondeo; `fm1ctl mix`
imprime lo mismo.

**Salida (cumplida):** girar `SIZE`, `MIX`, `DJ FILT` o el filtro de la pista en el aparato mueve la
franja correspondiente en la ventana en ≤1 s, sin bloquear el dibujo.

## M4 — Laboratorio de perfiles (en curso)

`fm1lab` deja de ser una app de JIANT: detecta el firmware y elige un **perfil** (`generic`,
`jiant-par12`, o uno del usuario en `~/.config/fm1lab/profiles/`). Un perfil declara parámetros con su
acceso (`cc`/`nrpn`/`editor`), transporte y escenas; las franjas se asignan desde el catálogo y se
guardan por perfil. ADR-0008.

**Hecho el 2026-11-13:** modelo de perfil, built-ins, `fm1-store` (perfiles + `panel.json`), acceso
CC/NRPN/editor, detección por identidad/INFO, picker, UI por capacidades, **perfil `x0x-1.2`** con el mapa CC del manual
y **monitor/learn** (`m`/`k`) que crea un parámetro desde el CC/NRPN que manda el aparato y lo guarda
en el perfil de usuario. El **selector de perfil** (`p`) cambia en caliente y `fm1ctl profile` lo
expone en la terminal; la elección se recuerda en `profile.json`. Verificado con X0X 1.2.2
(`x0x-1.2` detectado, 112 params).

**Falta:** perfiles de TRIBE/SLOOP (el learn ya permite construirlos en vivo), `params --profile` y la
prueba de learn/transporte con una perilla real. La prueba en vivo de JIANT queda pendiente de volver a
instalarlo (M5 ya permite hacerlo desde la app).

**Salida:** mover una perilla de un firmware desconocido, capturarla con el monitor, asignarla a una
franja y que quede guardada sin tocar código.

## M5 — Biblioteca e instalación de firmwares (en curso)

El lab deja de depender del instalador de `m-vave`: catálogo buscable, verificación SHA-256, backup y
flasheo OTA desde la ventana. ADR-0009.

**Hecho el 2026-11-13:** `fm1-flash` (port del protocolo Felucca), `fm1ctl flash`, catálogo built-in
`firmwares.json` + usuario, `fm1-store` con SHA-256 propio y `backup_lab()`, ventana `f` con búsqueda,
estado del paquete, identidad instalada, confirmación y log, y `fm1ctl firmware`. Al terminar el flash
la app re-detecta el perfil y lo cambia en caliente. Verificado en seco: X0X, JIANT par12/gen2, dock y
stock parsean y hashean; el stock exige `--force`.

**Falta:** el primer flash real con el instalador Rust (X0X 1.2.2 como prueba de ida y vuelta), el
catálogo remoto (hoy local + usuario) y probar la ventana con el aparato conectado. El Python de
`m-vave` sigue como rescate.

**Salida:** desde la ventana, buscar un firmware, ver que el SHA coincide, respaldar, instalar y que el
aparato reinicie con la identidad esperada; el stock/JIANT quedan a un click de vuelta.

## M6 — El estándar, publicado

Extraer de este repo la plantilla reutilizable: `AGENTS.md` + `docs/` + `scripts/check.sh` +
`.agents/skills/` + ADR de arranque, y un `cargo generate`-style seed para apps de escritorio livianas
en Rust (sin webview, puerta única, specs en openspec).

**Salida:** un repo nuevo creado desde la plantilla pasa su propia puerta en verde sin editar nada más
que el nombre.

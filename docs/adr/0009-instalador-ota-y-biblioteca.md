---
id: adr-0009
title: Instalador OTA en Rust y biblioteca de firmwares con backup
status: aprobada
date: 2026-11-13
supersedes: parte de 0008 (el flasheo ya no queda fuera del lab)
---

# 0009 — Instalar firmwares desde el lab, con checksum y backup

## Contexto

ADR-0008 dejó el flasheo fuera de `fm1lab`: se hacía con el instalador Python de `m-vave`
(`x0x/fm1_install.py`, `jiant/…`) y el lab sólo leía/escribía MIDI. El objetivo del producto cambió:
probar firmwares implica **cambiarlos seguido**, y hacerlo desde una terminal con un script aparte
rompe el flujo del laboratorio. La biblioteca también tiene que decir qué hay instalado, qué versión
está disponible y guardar una vuelta atrás.

El protocolo OTA/Felucca está descripto por el instalador de `m-vave` (handshake, loader, pedidos de
lectura del image lógico, verificación de identidad). Es chico y determinista, pero es la operación
más riesgosa del producto: escribe la flash y no hay ACK de escritura.

## Decisión

1. **Portar el instalador a Rust** en un crate aparte, `fm1-flash`, sin depender del Python en
   runtime. La CLI (`fm1ctl flash`) y la ventana usan el mismo camino.
2. **La biblioteca es dato**: `firmwares.json` en la raíz, embebido en el binario, más
   `~/.config/fm1lab/firmwares.json` para el usuario (gana por `id`). Cada entrada declara identidad
   OTA, archivo, SHA-256, etiquetas, notas y perfil sugerido. Los `.fwsc` se resuelven contra
   `FM1LAB_FIRMWARES`, `~/Proyectos/m-vave/firmware` o `~/.config/fm1lab/firmwares`.
3. **Verificar antes de escribir**: el SHA-256 del paquete se compara con el del catálogo; un paquete
   sin loader de Felucca (stock V16) exige `--force` explícito.
4. **Respaldar antes de escribir**: `setlist.json`, `panel.json`, `firmwares.json`, los perfiles de
   usuario y todos los `.fwsc` van a `~/.config/fm1lab/backups/<AAAAMMDD-HHMMSS>/`. Si el backup falla,
   no se escribe.
5. **Nunca instalar en silencio**: la ventana exige marcar la confirmación y la CLI exige `--yes`. La
   app para el transporte, pausa el sondeo (`Probe`) y el monitor, y recién ahí lanza el flash en un
   hilo con progreso y log.
6. **SHA-256 propio** (`fm1-store/src/sha256.rs`) en vez de una dependencia nueva: son ~70 líneas, con
   vectores conocidos en `cargo test`. No es criptografía de secretos.

El instalador Python de `m-vave` **no se borra**: queda como camino de rescate si un OTA se corta y el
aparato queda en modo update.

## Consecuencias

- `fm1-flash` es un adaptador: no decide cuándo instalar ni confirma por su cuenta; recibe un
  `Package` ya validado y devuelve la identidad final.
- La ventana `f` muestra catálogo, estado del paquete, identidad instalada, confirmación, progreso y
  log; `fm1ctl firmware` hace lo mismo en la terminal.
- La detección de “instalado” usa la identidad OTA exacta; JIANT par12 y gen2 comparten `FM-1_909`, así
  que la UI aclara que no puede distinguirlos.
- Después de un flash hay que reiniciar la app para que la detección elija el perfil nuevo; hacerlo en
  caliente queda como deuda.
- El catálogo es local: no hay descarga remota ni firma de catálogo. El día que haya hub, se firma.
- El primer flash real con el instalador Rust es la verificación pendiente; hasta entonces, `--dry`
  sólo prueba parseo, checksum y bytes.

## Alternativas descartadas

- **Invocar el Python de `m-vave`.** Depende de Python, `mido` y `python-rtmidi`; el binario deja de ser
  autocontenido y el error se reparte entre dos runtimes.
- **Instalar desde el instalador web.** Requiere navegador y WebMIDI; contradice ADR-0002 (sin webview)
  y no permite backup/estado en el lab.
- **No flashear desde el lab.** Es el statu quo de ADR-0008 y obliga a salir del producto para la
  operación central de “probar firmwares”.
- **Sumar `sha2` para el checksum.** Una dependencia más para 70 líneas; se prefiere el hash propio con
  tests conocidos.

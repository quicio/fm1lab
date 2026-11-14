## Contexto

El instalador probado vive en `m-vave/x0x/fm1_install.py` (GPL-3.0): paso 1, el firmware corriendo lee
partes del paquete y arranca el loader; paso 2, el loader lee el paquete completo y lo escribe. En los
dos pasos el aparato **pide** lecturas del image lógico por SysEx y el host responde. Al final
reinicia y se verifica la identidad. No hay ACK de escritura: la única prueba es la identidad final.

## Decisiones

1. **Crate `fm1-flash`**: adaptador puro. `Package::load` valida el `.fwsc` y arma el image lógico;
   `Installer::install` hace handshake → check → loader → write → verify, emitiendo `Event` para la UI.
   No decide cuándo instalar ni hace backup: eso es de la app/CLI.
2. **Catálogo como dato**: `firmwares.json` en la raíz, `include_str!` en `fm1-store::catalog`; el
   usuario pisa por `id` desde `~/.config/fm1lab/firmwares.json` (`FM1LAB_CATALOG`). Los paquetes se
   resuelven con `FM1LAB_FIRMWARES`, `~/Proyectos/m-vave/firmware` o `~/.config/fm1lab/firmwares`.
3. **SHA-256 propio**: 70 líneas en `fm1-store/src/sha256.rs`, con vectores conocidos (vacío, `abc`,
   mensaje de 56 bytes) y un test archivo-vs-memoria. Evita una dependencia nueva para integridad.
4. **Backup**: `backup_lab_at` copia config, perfiles y los `.fwsc` del catálogo a
   `backups/<AAAAMMDD-HHMMSS>/`. Es best-effort por archivo, pero si no se puede crear el backup la
   instalación se aborta antes de escribir.
5. **Pausa**: `Probe` y `Monitor` tienen `pause()`/`resume()`; `Monitor` suelta el puerto de entrada.
   La ventana para el transporte y llama `start_install` recién después.
6. **Confirmación**: la ventana tiene un checkbox y un botón `instalar`; la CLI exige `--yes`. El
   `--force` sólo destraba paquetes sin loader de Felucca (stock V16) y se avisa en la ficha.
7. **Hilo**: el flash corre en un hilo con `mpsc`; la UI consume `Line`/`Progress`/`Flashed` y no se
   bloquea. Al terminar, el resultado queda visible, se re-detecta y se cambia de perfil en caliente.

## Flujo de instalación

```
usuario confirma
  → parar transporte / pausar Probe + Monitor
  → backup config + perfiles + .fwsc        (si falla: abortar)
  → Package::load (identidad + loader)      (si falla: abortar)
  → Installer::install
       handshake (identidad OTA)
       check  (firmware corre el loader)
       write  (loader escribe el image)
       verify (reinicio + identidad esperada)
  → estado: instalado · identidad
```

## Riesgos y mitigación

| riesgo | mitigación |
|---|---|
| Escritura interrumpida | Backup previo + loader queda en modo update; el Python de `m-vave` termina la escritura |
| Paquete equivocado o corrupto | Identidad del paquete vs. modelo del aparato; SHA-256 del catálogo |
| Otro cliente MIDI interfiere | Pausa de `Probe`/`Monitor` y transporte parado |
| El usuario no sabe qué va a pasar | Confirmación explícita, ficha con identidad/perfil y log en vivo |
| Stock V16 sin loader | no se ofrece por OTA: M-UPGRADE; la ruta lo rechaza |

## No-goals

- Catálogo remoto firmado (queda para cuando haya un hub).
- Descarga remota del catálogo y firma de paquetes.
- Backup del proyecto/sonidos del firmware por el editor: hoy sólo config y paquetes.

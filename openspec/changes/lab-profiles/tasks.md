## 1. Modelo y perfiles

- [x] 1.1 `fm1-core`: `Profile`, `Param`, `ParamAccess`, `TransportSpec`, `SceneSpec`, `Scope` con serde.
- [x] 1.2 Perfil built-in `generic` (CC estándar de 6 pistas) y `jiant-par12` generado desde la tabla.
- [x] 1.3 `fm1-store`: cargar perfiles de usuario de `~/.config/fm1lab/profiles/` y validar; built-in como fallback.
- [x] 1.4 Tests: perfil válido/inválido, merge usuario/built-in, `generic` tiene las 6 pistas.

## 2. Acceso y detección

- [x] 2.1 `fm1-midi`: `read_param`/`write_param`/`watch` por `ParamAccess` (editor/CC/NRPN).
- [x] 2.2 Detección por identidad OTA + INFO; `--profile`; fallback `generic`.
- [x] 2.3 Tests de acceso CC/NRPN con `Recorder`/guion; editor con las capturas actuales.

## 3. App por capacidades

- [x] 3.1 Cargar/activar perfil; cabecera y selector.
- [x] 3.2 Franjas desde el perfil; `a` abre el picker; asignación persistida por perfil.
- [x] 3.3 Transporte y setlist sólo si el perfil los declara; JIANT conserva lo actual.
- [x] 3.4 `--shot --demo` con el perfil genérico y con JIANT.

## 4. Monitor y learn (fase 2)

- [x] 4.1 Monitor de entrada con snapshot y sin bloquear el dibujo.
- [x] 4.2 Learn de CC/NRPN → parámetro en el perfil de usuario (JSON atómico).
- [x] 4.3 Tests del parser de CC/NRPN y del perfil aprendido.

## 5. CLI, docs y verificación

- [x] 5.1 `fm1ctl profile [id]` (detección + detalle), selector `p` y `profile.json` (último perfil).
- [ ] 5.1b `params --profile` (el mapa NRPN sigue siendo el de JIANT).
- [x] 5.2 `PRODUCT.md`, `AGENTS.md`, `HANDOFF.md`, `ARCHITECTURE.md`, ADR-0008.
- [x] 5.3 `scripts/check.sh` verde con perfil genérico y JIANT.
- [ ] 5.4 Prueba en vivo: JIANT paridad; `generic` con CC en JIANT/stock.

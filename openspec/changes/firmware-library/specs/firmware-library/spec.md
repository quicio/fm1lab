## Purpose

Permite buscar, verificar e instalar firmwares del FM-1 desde el lab, con backup y confirmación
explícita, y saber qué firmware está instalado.

## ADDED Requirements

### Requirement: Catálogo buscable

El lab SHALL ofrecer un catálogo de firmwares con nombre, versión, familia, identidad OTA, archivo,
SHA-256, etiquetas y notas. El catálogo built-in SHALL viajar con el binario y el usuario SHALL poder
sumar o reemplazar entradas por `id` desde `~/.config/fm1lab/firmwares.json`. La búsqueda SHALL cubrir
nombre, id, versión, familia, etiquetas y notas.

#### Scenario: Buscar por etiqueta

- **WHEN** el usuario escribe `acid` en la biblioteca
- **THEN** aparece el firmware X0X y no los que no declaran esa etiqueta

#### Scenario: Entrada de usuario

- **WHEN** el usuario guarda una entrada con el mismo `id` que una built-in
- **THEN** la del usuario gana y el built-in no se duplica

### Requirement: Estado del paquete

El lab SHALL informar si el archivo existe, si su SHA-256 coincide con el declarado y qué identidad
OTA reporta el aparato conectado. Un paquete ausente, ilegible o con hash distinto NO SHALL poder
instalarse.

#### Scenario: Hash distinto

- **WHEN** el archivo cambió y el SHA-256 no coincide
- **THEN** la ficha lo muestra en rojo y el botón de instalar queda deshabilitado

#### Scenario: Firmware instalado

- **WHEN** la identidad OTA del aparato coincide con una entrada
- **THEN** la lista la marca como instalada (`*` en la CLI, `●` en la ventana)

### Requirement: Backup antes de escribir

Antes de cualquier escritura, el lab SHALL copiar `setlist.json`, `panel.json`, `firmwares.json`, los
perfiles de usuario y los `.fwsc` del catálogo a `~/.config/fm1lab/backups/<fecha>/`. Si el backup no
se puede crear, NO SHALL escribir la flash.

#### Scenario: Backup correcto

- **WHEN** el usuario confirma una instalación
- **THEN** existe una carpeta nueva en `backups/` con la config y los paquetes antes del handshake

#### Scenario: Backup falla

- **WHEN** no se puede crear la carpeta de backup
- **THEN** la instalación se aborta y el aparato queda intacto

### Requirement: Instalación con confirmación y pausa

La instalación SHALL requerir confirmación explícita (`--yes` en la CLI, checkbox en la ventana). Antes
del OTA el lab SHALL parar el transporte y pausar el sondeo y el monitor. El resultado SHALL mostrar la
identidad final; si no coincide con la del paquete, SHALL reportarlo como error.

#### Scenario: Confirmación faltante

- **WHEN** se pide `fm1ctl firmware --install x0x-1.2.2` sin `--yes`
- **THEN** el comando no escribe y explica que falta la confirmación

#### Scenario: Instalación correcta

- **WHEN** el usuario confirma y el paquete está listo
- **THEN** el lab responde `instalado · <identidad>` y re-detecta el perfil; si el aparato tarda en enumerar, sugiere reiniciar

#### Scenario: Paquete sin loader

- **WHEN** el paquete no trae el loader de Felucca (stock V16)
- **THEN** el lab lo rechaza salvo `--force`/checkbox explícito y lo advierte en la ficha

### Requirement: CLI equivalente

El lab SHALL exponer `fm1ctl firmware [query]` para listar y `fm1ctl firmware --install <id>` para
instalar, con `--dry` para validar sin escribir.

#### Scenario: Validar sin aparato

- **WHEN** se corre `fm1ctl firmware --install x0x-1.2.2 --dry`
- **THEN** imprime identidad, bytes y estado del paquete sin abrir puerto ni escribir

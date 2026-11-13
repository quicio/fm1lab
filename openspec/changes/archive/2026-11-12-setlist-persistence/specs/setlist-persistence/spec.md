## Purpose

Permite guardar el set de un show en un archivo portable, volver a abrirlo tal como estaba y
compartirlo, sin depender de lo que tenga cargado el aparato en ese momento.

## ADDED Requirements

### Requirement: Formato versionado y guardado atómico

El setlist SHALL guardarse en JSON con un número de versión y los pasos del show. El guardado SHALL
ser atómico: escribir un temporal en el mismo directorio, sincronizarlo y renombrarlo sobre el destino.
Un archivo existente no SHALL quedar truncado si el guardado falla.

#### Scenario: Guardar y reabrir un show de 12 pasos

- **WHEN** un setlist de 12 pasos se guarda y se vuelve a cargar
- **THEN** el nombre, el orden y cada paso coinciden con los originales

#### Scenario: Fallo a mitad de escritura

- **WHEN** el guardado no puede completarse
- **THEN** el archivo anterior sigue siendo el válido y no queda un temporal con el nombre del destino

### Requirement: Validación al cargar

Al cargar, el setlist SHALL rechazar una versión desconocida y cualquier paso fuera de rango, y SHALL
informar el error en vez de usar datos dudosos.

#### Scenario: Paso inválido

- **WHEN** el archivo trae una sección `Z` o un proyecto fuera de rango
- **THEN** la carga falla con un mensaje y no se dispara ningún paso

### Requirement: Archivo corrupto

Si el JSON no se puede leer, el store SHALL mover el archivo a un `.bak` y SHALL informar el error;
no SHALL sobrescribirlo en silencio.

#### Scenario: JSON roto

- **WHEN** el archivo no es JSON válido
- **THEN** queda una copia `.bak` y la app muestra que la carga falló

### Requirement: Plantilla de fábrica

Si no hay archivo, el store SHALL ofrecer una plantilla de fábrica válida y SHALL poder escribirla a
pedido.

#### Scenario: Primera vez

- **WHEN** el usuario pide inicializar un setlist y el archivo no existe
- **THEN** se escribe la plantilla, con secciones A–D, y se puede cargar y disparar

### Requirement: La app usa el archivo o el aparato, a elección

La app SHALL cargar el archivo al arrancar. Con un archivo cargado, una lectura automática del aparato
no SHALL reemplazar el set; `r` SHALL cambiar al set del aparato y `s` SHALL guardar el set actual en
el archivo.

#### Scenario: Arrancar con un archivo guardado

- **WHEN** existe el setlist y la app arranca
- **THEN** muestra ese set y no lo pisa la lectura del aparato

#### Scenario: Traer el set del aparato

- **WHEN** el usuario aprieta `r`
- **THEN** la app muestra el set del aparato y puede guardarlo con `s`

### Requirement: Verificación sin abrir la UI

La CLI SHALL poder validar un archivo, listar sus pasos y, con `--dry`, imprimir los bytes que
mandaría cada paso.

#### Scenario: Inspeccionar un archivo

- **WHEN** se ejecuta `fm1ctl setlist show.json --dry`
- **THEN** imprime los 12 pasos y los bytes de cada uno, sin abrir ningún puerto

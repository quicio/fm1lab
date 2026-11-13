# device-read Specification

## Purpose
Hace que el panel muestre y dispare lo que el aparato tiene cargado de verdad, en vez de una
setlist de ejemplo que puede apuntar a proyectos vacíos.
## Requirements
### Requirement: Setlist desde el contenido del aparato

Al arrancar y cuando el usuario pide releer, la app SHALL consultar las cuatro secciones del
proyecto cargado, la cadena de song y el tempo real. SHALL armar el set con las secciones que tienen
música, en el orden de la cadena cuando exista, y SHALL omitir las secciones vacías.

#### Scenario: Proyecto con tres secciones guardadas

- **WHEN** el proyecto cargado tiene las secciones A, B y C guardadas y D vacía
- **THEN** el set muestra A, B y C, y no muestra D

#### Scenario: Cadena de song configurada

- **WHEN** la cadena del song tiene filas A, B, C, D y D está vacía
- **THEN** el set respeta el orden de la cadena y descarta la fila D

#### Scenario: Sin cadena

- **WHEN** el song no tiene filas configuradas
- **THEN** el set lista las secciones guardadas en orden A, B, C, D

### Requirement: El tempo arranca con el del aparato

La app SHALL leer el BPM del aparato **una sola vez**, al armar el set por primera vez, y usarlo como
tempo inicial del reloj. Después de eso el tempo es de la app: una relectura no lo cambia y sólo lo
mueve el usuario.

#### Scenario: Aparato a 96 BPM

- **WHEN** el aparato está en 96 BPM y la app arma el set
- **THEN** el indicador de tempo de la app muestra 96 y el reloj que manda al reproducir es 96

#### Scenario: Relectura con la app reproduciendo

- **WHEN** la app está reproduciendo y una relectura del aparato trae un BPM distinto (el firmware
  estima el tempo de nuestros pulsos)
- **THEN** el tempo de la app no cambia y el reloj sigue estable

### Requirement: La lectura no bloquea el dibujo

La lectura del aparato SHALL ocurrir fuera del hilo de dibujo. Mientras no haya respuesta, la app
SHALL seguir respondiendo y mostrando el estado anterior.

#### Scenario: Aparato lento o ausente

- **WHEN** el aparato no contesta o está desconectado
- **THEN** la ventana sigue respondiendo y muestra que no hay aparato o que la lectura falló

### Requirement: Releer a pedido

La app SHALL permitir volver a leer el aparato sin reiniciarse, para tomar cambios hechos en el
panel del FM-1 (otro proyecto, otra cadena, otro tempo).

#### Scenario: Cambiar de proyecto en el aparato

- **WHEN** el usuario carga otro proyecto en el aparato y pide releer
- **THEN** el set se actualiza con el contenido del proyecto nuevo; el tempo de la app se mantiene
  (se ajusta con `±`)


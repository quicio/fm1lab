## Purpose

Hace que la ventana refleje los valores que tiene el aparato en ese momento, para que mover una
perilla en el FM-1 se vea en el panel sin tener que releer a mano.

## ADDED Requirements

### Requirement: Seguimiento de las franjas

La app SHALL consultar periódicamente la pista seleccionada y los cuatro valores de las franjas por el
editor protocol, y SHALL reflejar un cambio hecho en el aparato en un segundo o menos. La consulta
SHALL ocurrir fuera del hilo de dibujo.

#### Scenario: Girar una perilla en el aparato

- **WHEN** el usuario gira `SIZE` (o `MIX`, `DJ FILT`, o el filtro de la pista) en el aparato
- **THEN** la franja correspondiente de la ventana muestra el valor nuevo en ≤1 s

#### Scenario: Aparato desconectado

- **WHEN** el aparato deja de contestar
- **THEN** la app conserva los últimos valores, no bloquea la ventana y reintenta en el próximo sondeo

### Requirement: La pista seleccionada manda en el filtro

La franja de filtro SHALL mostrar el valor de la pista seleccionada en el aparato y SHALL rotularse
con el número de esa pista.

#### Scenario: Cambiar de pista en el aparato

- **WHEN** el aparato selecciona otra pista
- **THEN** la franja cambia de rótulo (`cabezal tN`) y muestra el filtro de esa pista

### Requirement: No pelear con el cambio local

Un valor recién movido con `↑↓` en la app no SHALL ser pisado por un sondeo anterior que llegue tarde.

#### Scenario: Mover una franja y soltar

- **WHEN** el usuario mueve una franja y el sondeo anterior trae el valor viejo
- **THEN** la app mantiene el valor nuevo y el siguiente sondeo confirma el del aparato

### Requirement: El seguimiento no toca el set ni el tempo

El snapshot de las franjas SHALL NOT cambiar el setlist ni el tempo de la app.

#### Scenario: Perilla mientras suena el set

- **WHEN** el usuario gira una perilla en el aparato mientras corre el set
- **THEN** la franja se actualiza y el set y el tempo siguen como estaban

### Requirement: Verificación sin abrir la UI

La CLI SHALL poder imprimir la pista seleccionada y los cuatro valores actuales.

#### Scenario: Ver el estado

- **WHEN** se ejecuta `fm1ctl mix`
- **THEN** imprime la pista y los cuatro valores leídos del aparato

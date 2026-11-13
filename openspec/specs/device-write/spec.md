# device-write Specification

## Purpose
Que mover una franja o el tempo en la app cambie el aparato que está corriendo, usando el protocolo
que el firmware efectivamente implementa y no uno que sólo está documentado.
## Requirements
### Requirement: El tempo y las franjas se escriben por el editor protocol

La app SHALL escribir el tempo y los parámetros de las cuatro franjas por el editor protocol, con el
id y el rango que publica el mapa y el valor crudo en offset binary. No SHALL depender de NRPN para
esos cambios.

#### Scenario: Mover el filtro de la pista

- **WHEN** el usuario mueve la franja `cabezal` con `↑` o `↓`
- **THEN** el valor del filtro en el aparato cambia y una lectura posterior devuelve el mismo valor

#### Scenario: Cambiar el tempo

- **WHEN** el usuario cambia el tempo con `±`
- **THEN** el BPM que devuelve una lectura del aparato es el que muestra la app

#### Scenario: Mover un global

- **WHEN** el usuario mueve `dj filter`, `reverb size` o `mix`
- **THEN** el global correspondiente del aparato cambia y se puede leer de vuelta

### Requirement: La franja de pista dice sobre cuál actúa

La franja de filtro SHALL actuar sobre la pista seleccionada en el aparato y SHALL mostrar cuál es,
leída del aparato, en vez de suponer que es la pista 1.

#### Scenario: El aparato tiene otra pista seleccionada

- **WHEN** el aparato tiene seleccionada la pista T3 y la app lee el estado
- **THEN** la franja se rotula `cabezal t3` y escribe el filtro de esa pista


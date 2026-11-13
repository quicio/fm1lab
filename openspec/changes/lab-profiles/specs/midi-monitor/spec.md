## Purpose

Permite ver el MIDI que manda el aparato y capturar un control (CC o NRPN) que el usuario mueva para
agregarlo a un perfil, sin editar JSON a mano ni recompilar.

## ADDED Requirements

### Requirement: Monitor de entrada

El lab SHALL mostrar los últimos mensajes MIDI que llegan del aparato, con su tipo, canal, números y
bytes, y SHALL hacerlo sin bloquear el dibujo.

#### Scenario: Girar una perilla desconocida

- **WHEN** el firmware manda un CC o NRPN al mover una perilla
- **THEN** el monitor muestra el mensaje con sus bytes y canal

### Requirement: Aprender un control

En modo aprender, el lab SHALL capturar el próximo CC o NRPN que llegue y SHALL ofrecer asignarlo a la
franja enfocada, creando el parámetro en el perfil de usuario.

#### Scenario: Aprender LEVEL de una pista

- **WHEN** el usuario arma aprender, mueve el LEVEL de la pista 2 en el aparato y confirma
- **THEN** el perfil de usuario gana un parámetro `T2 LEVEL` accesible por CC y la franja lo controla

### Requirement: El perfil aprendido se guarda

El lab SHALL guardar el perfil de usuario con la misma escritura atómica que el resto de la
configuración y SHALL cargarlo al arrancar.

#### Scenario: Reinicio

- **WHEN** el usuario aprende un control y reinicia el lab
- **THEN** el parámetro aprendido sigue disponible en el perfil

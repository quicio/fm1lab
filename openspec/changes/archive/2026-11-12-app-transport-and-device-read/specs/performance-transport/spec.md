## Purpose

Permite que el panel de performance arranque y pare la reproducción del FM-1 desde la computadora,
sin depender de que alguien apriete PLAY en el aparato, y que al parar lo deje como estaba.

## ADDED Requirements

### Requirement: Play desde la app como reloj maestro

Mientras reproduce, la app SHALL marcar el reloj: leer el ajuste `CLOCK` vigente, ponerlo en `USB`,
enviar `Start` y enviar 24 pulsos de clock MIDI por negra al tempo que muestra la app. El primer
pulso SHALL salir dentro de una negra desde el `Start`.

#### Scenario: Aparato parado con reloj interno

- **WHEN** el aparato está parado, con `CLOCK = INT`, y el usuario da play en la app
- **THEN** el aparato cambia a `CLOCK = USB`, arranca en el paso cero y suena al tempo de la app

#### Scenario: Play con la app ya reproduciendo

- **WHEN** el usuario vuelve a dar play
- **THEN** la app no reinicia el transporte ni duplica el hilo de clock

### Requirement: Stop restaura el reloj del aparato

Al parar, la app SHALL enviar `Stop`, terminar los pulsos de clock, dejar el BPM del aparato en el
tempo que estuvo usando y restaurar el valor de `CLOCK` que leyó antes de empezar. La restauración
SHALL ocurrir también si el envío de clock falla o si la app se cierra con el transporte andando.

#### Scenario: Parada normal

- **WHEN** el usuario para desde la app
- **THEN** el aparato deja de sonar, su BPM queda en el tempo usado y su `CLOCK` vuelve al valor que
  tenía (por ejemplo `INT`)

#### Scenario: Se corta el enlace

- **WHEN** un envío de clock falla porque el aparato se desconectó
- **THEN** la app deja de reproducir, muestra el error e intenta restaurar el `CLOCK`

### Requirement: Disparar arranca si está parado

Disparar un paso del set SHALL cargar primero la sección o el proyecto y después arrancar la
reproducción si estaba parada, de modo que el primer compás sea el paso disparado. Si ya estaba
sonando, SHALL sólo pedir el cambio, que el firmware aplica al compás siguiente.

#### Scenario: Primer disparo con el aparato parado

- **WHEN** el aparato está parado y el usuario dispara la sección B
- **THEN** la sección B queda cargada y la reproducción arranca en B

#### Scenario: Disparo con la reproducción andando

- **WHEN** la app está reproduciendo la sección A y el usuario dispara la sección C
- **THEN** la app no reinicia el transporte y el aparato entra en C al compás siguiente

### Requirement: El estado de transporte es visible

La app SHALL mostrar de forma explícita si está reproduciendo o parada, y SHALL permitir alternar
con una tecla. El estado SHALL ser el de la app, no una suposición derivada de un envío exitoso.

#### Scenario: Alternar con el teclado

- **WHEN** el usuario aprieta la tecla de transporte
- **THEN** la app pasa de parada a reproduciendo (o al revés) y lo refleja en pantalla

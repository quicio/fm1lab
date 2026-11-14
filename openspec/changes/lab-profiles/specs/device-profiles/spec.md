## Purpose

Permite que el laboratorio reconozca distintos firmwares del FM-1, sepa qué parámetros y acciones
expone cada uno y adapte la interfaz sin recompilar la app por cada firmware.

## ADDED Requirements

### Requirement: Perfil declarativo por firmware

El lab SHALL describir cada firmware con un perfil que declare identidad, parámetros (nombre, rango,
default y acceso), transporte y escenas. Un perfil SHALL poder ser un archivo JSON; los perfiles
built-in (`generic`, `jiant-par12`, `x0x-1.2`, `sloop-2.8`) SHALL venir con el binario.

#### Scenario: Firmware no reconocido

- **WHEN** el aparato contesta una identidad que ningún perfil reconoce
- **THEN** el lab usa el perfil `generic` (CC estándar de 6 canales) y lo muestra en pantalla

#### Scenario: Perfil forzado

- **WHEN** el usuario arranca con `--profile jiant-par12`
- **THEN** el lab usa ese perfil aunque la detección diga otra cosa

#### Scenario: Elegir perfil en la ventana

- **WHEN** el usuario elige un perfil con `p`
- **THEN** el lab cambia franjas, setlist y transporte, y recuerda la elección en `profile.json`

#### Scenario: Sin aparato

- **WHEN** no hay FM-1 conectado y existe un último perfil guardado
- **THEN** el lab arranca con ese perfil en vez de caer a `generic`

### Requirement: Acceso a parámetros según el protocolo

El lab SHALL leer y escribir un parámetro según su acceso: `editor`, `cc` o `nrpn`. Cuando el
protocolo permita confirmar (editor), SHALL esperar la respuesta y avisar si el valor aplicado no es
el pedido.

#### Scenario: Parámetro de editor

- **WHEN** se mueve una franja de un parámetro `editor` de JIANT
- **THEN** el lab manda el `SET` y espera la confirmación del aparato

#### Scenario: Parámetro CC

- **WHEN** se mueve una franja de un parámetro `cc` de un firmware genérico
- **THEN** el lab manda el CC en el canal y número declarados

### Requirement: La interfaz se adapta a las capacidades

El lab SHALL mostrar el transporte y el setlist/escenas sólo si el perfil los declara, y SHALL ocultar
lo que el firmware no soporta.

#### Scenario: Perfil genérico sin secciones

- **WHEN** el perfil activo no declara escenas por Program Change
- **THEN** el panel no muestra la lista de secciones/proyectos

#### Scenario: Perfil JIANT

- **WHEN** el perfil activo es `jiant-par12`
- **THEN** el panel muestra secciones, proyectos, reloj USB y la lectura del editor como hoy

### Requirement: Perfiles de usuario

El lab SHALL cargar perfiles extra desde `~/.config/fm1lab/profiles/` y SHALL preferir el perfil de
usuario cuando tenga el mismo id que uno built-in.

#### Scenario: Perfil propio

- **WHEN** el usuario agrega `x0x.json` en su carpeta de perfiles
- **THEN** el perfil aparece en la lista y puede elegirse sin recompilar

# PRODUCT

## Qué es

Un **laboratorio de performance** para el M-VAVE FM-1: conectar el aparato, reconocer qué firmware
tiene, describir lo que expone (parámetros, transporte, escenas) y dar un panel chico para tocarlo y
explorarlo desde la computadora.

No es “la app de JIANT”: JIANT es **un perfil** más.

## Para quién

Para quien experimenta con firmwares del FM-1 (JIANT, X0X, TRIBE, SLOOP, stock, dock) y quiere un panel
liviano para mover parámetros, armar franjas a mano, ver el MIDI y probar sin abrir un DAW ni
recompilar la app por cada firmware.

## Qué tiene que hacer sí o sí

- **Detectar** el firmware (identidad/INFO) y elegir un **perfil**; si no lo conoce, caer a un perfil
  genérico de CC estándar.
- **Describir** el firmware con datos: parámetros, acceso (CC/NRPN/editor), transporte y escenas; los
  perfiles de usuario se agregan sin recompilar y se eligen con `p` (se recuerda el último).
- **Asignar** cualquier parámetro del perfil a las cuatro franjas y guardarlo por perfil.
- **Escribir y leer** por el protocolo que el perfil declare, confirmando cuando el protocolo lo
  permita.
- **Mostrar sólo lo que el firmware soporta** (transporte, secciones/escenas).
- **Monitorear y aprender** un CC/NRPN en vivo para sumarlo al perfil.
- **Buscar e instalar firmwares** desde una biblioteca con checksum, backup previo y confirmación
  explícita; el stock y el resto de los paquetes quedan a mano para volver.
- Entrar en un binario chico y abrir al instante: es una herramienta de escenario/lab, no una suite.

## Límites

- No es un editor de sonidos completo: para presets y motores están el editor web del firmware y el
  aparato.
- **Flashear es una operación deliberada**, no un atajo: la app verifica el SHA-256, respalda
  configuración y paquetes, pausa transporte/sondeo/monitor y pide confirmación; nunca instala en
  silencio. El instalador Python de `m-vave` sigue siendo el camino de rescate.
- No es un DAW: no secuencia audio ni plugins; manda MIDI y lee lo que el aparato expone.
- La UI es macOS/Linux/Windows vía `midir`; el dibujo es egui sin webview (ADR-0002).

## Referencia del protocolo

Cada firmware tiene su contrato. El perfil es la fuente: identidad, parámetros, accesos, transporte y
escenas. `generic` cubre el mapa CC estándar del FM-1 (6 pistas); `jiant-par12` cubre el editor
SysEx, las secciones/proyectos y el reloj USB; `x0x-1.2` cubre los 112 CC del manual de X0X. Lo que no
esté en un perfil se puede aprender desde el monitor. La biblioteca de firmwares es `firmwares.json`
(built-in + usuario) y se instala con `fm1-flash` (OTA/Felucca).

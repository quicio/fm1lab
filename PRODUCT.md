# PRODUCT

## Qué es

Una app de escritorio para operar un **M-VAVE FM-1** en vivo desde la computadora: correr un set
(secciones y proyectos), mover los parámetros que importan (tempo, filtros, efectos, mezcla) y no tocar el
aparato con los dedos en pleno show.

## Para quién

Para quien ya usa el FM-1 con un firmware abierto (JIANT, Felucca, fm1-dock, stock) y quiere un panel de
control sin abrir un DAW. Perfil real: alguien que arma sets, toca en vivo y prefiere teclas y listas
cortas antes que ventanas con formularios.

## Qué tiene que hacer sí o sí

- Disparar sección y proyecto con una tecla, con la latencia de un byte MIDI (no de un frame web).
- Mostrar el estado mínimo: aparato conectado, qué canal, qué sección está sonando.
- Funcionar sin el aparato abierto (la lista se edita igual, se avisa que no hay conexión).
- Entrar en un binario chico y abrir al instante: es una herramienta de escenario, no una suite.

## Límites

- No es un editor de sonidos: para presets y motores están el editor web del firmware y el aparato.
- No flashea firmware: esa operación vive en el proyecto `m-vave` con su propio procedimiento.
- No es multiplataforma de UI en M1, pero la capa MIDI sí (`midir` cubre macOS/Linux/Windows).

## Referencia del protocolo

El aparato expone todo por MIDI estándar: NRPN de 14 bits para parámetros, Program Change en el canal 16
para secciones y proyectos. Nada de esto requiere ingeniería inversa: lo documenta el firmware.

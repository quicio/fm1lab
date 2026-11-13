## Purpose

Deja que el usuario elija qué parámetros del perfil activo controla cada una de las cuatro franjas y
que esa elección sobreviva al reinicio.

## ADDED Requirements

### Requirement: Asignar un parámetro a una franja

El lab SHALL abrir una lista buscable con los parámetros del perfil activo y SHALL asignar el elegido a
la franja enfocada, tomando nombre y rango del perfil.

#### Scenario: Asignar SIZE

- **WHEN** el usuario aprieta `a` en la franja 3, busca `SIZE` y lo elige
- **THEN** la franja pasa a controlar `SIZE`, muestra su rango y su valor actual

### Requirement: Persistencia por perfil

La asignación SHALL guardarse por `profile_id` y SHALL cargarse al arrancar con ese perfil. Si un
parámetro guardado no existe en el perfil activo, el lab SHALL avisar y usar el default de esa franja.

#### Scenario: Cambiar de firmware

- **WHEN** el usuario pasa de JIANT a un perfil genérico con asignaciones guardadas de JIANT
- **THEN** las franjas que no existen en el perfil nuevo vuelven al default y se avisa, sin borrar el
  archivo de JIANT

### Requirement: Seguimiento de lo asignado

El sondeo SHALL leer los parámetros asignados que el perfil declare legibles, y SHALL actualizar las
franjas sin pisar un valor recién movido con `←→`.

#### Scenario: Perilla del aparato

- **WHEN** el usuario gira una perilla de un parámetro asignado y legible
- **THEN** la franja muestra el valor nuevo en ≤1 s

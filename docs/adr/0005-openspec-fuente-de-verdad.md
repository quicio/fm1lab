---
id: adr-0005
title: openspec gobierna los cambios; docs describe el estado
status: aprobada
date: 2026-11-11
supersedes: —
---

# 0005 — `openspec/` como fuente de verdad

## Contexto

El repo necesita dos cosas distintas: (a) un lugar donde un cambio se propone, se discute y se archiva, y
(b) documentación del estado actual para que un recién llegado sea productivo en una sesión. Confundirlas
produce specs que describen features a medias y `docs/` que mienten.

El proyecto hermano `candil` ya usa `openspec/` (`specs/` vigentes, `changes/archive/` histórico) y una
`docs/` con handoff, roadmap, arquitectura y ADR. La skill `agent-handoff-package` advierte
explícitamente: **si el repo ya tiene un sistema de specs, no construir un segundo**.

## Decisión

- `openspec/` — cambios: propuesta, tareas, validación (`openspec validate --specs --strict`).
- `docs/` — estado: handoff, roadmap, arquitectura.
- `docs/adr/` — decisiones ya cocinadas, en forma Nygard con alternativas.

Nada de specs en `docs/specs/`: sería el segundo árbol.

## Consecuencias

- Un cambio se lee en un solo lugar y su historia queda archivada.
- Los ADR no duplican specs: registran el porqué de una decisión que ya está en el código.
- `posta init` se corrió con `--no-specs`, justamente para no generar un árbol paralelo.

## Alternativas descartadas

- **`docs/specs/` como en `candil`.** Ahí conviven porque las specs son históricas (M1–M6) y el flujo
  nuevo pasó a openspec; acá arrancamos directamente con openspec.
- **Sólo ADR.** Sirve para decisiones, no para trabajo en curso.

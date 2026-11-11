#!/usr/bin/env python3
"""Genera crates/fm1-core/src/nrpn_table.rs desde docs/NRPN.md.

El firmware de JIANT genera su docs/NRPN.md desde firmware/src/params.c y falla un test
si divergen. Del lado de la app hacemos lo mismo: el .md es la fuente, la tabla es salida.
No editar el .rs a mano: correr este script (scripts/check.sh lo verifica).

    python3 scripts/gen_nrpn.py            # escribe el archivo
    python3 scripts/gen_nrpn.py --check    # falla si el archivo cambió (para CI)
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "docs" / "NRPN.md"
OUT = ROOT / "crates" / "fm1-core" / "src" / "nrpn_table.rs"

ROW = re.compile(r"^\|\s*(\d+)\s*\|\s*([^|]+?)\s*\|\s*(-?\d+)\s*\.\.\s*(-?\d+)\s*\|\s*(-?\d+)\s*\|")
ENGINE_ROW = re.compile(
    r"^\|\s*([A-Z0-9 ]+?)\s+(\d+)\s*\|\s*(\d+)\s*\|\s*([^|]+?)\s*\|\s*(-?\d+)\s*\.\.\s*(-?\d+)\s*\|\s*(-?\d+)\s*\|"
)


def parse_engine_rows(text: str) -> list[tuple[str, int, str, int, int, int]]:
    """Las filas (motor | CC98 | nombre | min..max | default) de la sección de motores."""
    body = text.split("## Engine parameters", 1)[1].split("## Globals", 1)[0]
    rows, seen = [], set()
    for line in body.splitlines():
        m = ENGINE_ROW.match(line)
        if not m:
            continue
        engine, pid, name = m.group(1).strip(), int(m.group(3)), m.group(4).strip().upper()
        key = (engine, pid, name)
        if key in seen:
            continue
        seen.add(key)
        rows.append((engine, pid, name, int(m.group(5)), int(m.group(6)), int(m.group(7))))
    return rows


def parse_rows(text: str, start: str, end: str) -> list[tuple[int, str, int, int, int]]:
    """Las filas (CC98 | nombre | min..max | default | valores) de la sección start..end."""
    body = text.split(start, 1)[1].split(end, 1)[0]
    rows, seen = [], set()
    for line in body.splitlines():
        m = ROW.match(line)
        if not m:
            continue
        pid, name, lo, hi, default = m.group(1), m.group(2), m.group(3), m.group(4), m.group(5)
        key = (int(pid), name.upper())
        if key in seen:            # un id repetido en varios motores: nos quedamos con el primero
            continue
        seen.add(key)
        rows.append((int(pid), name.upper(), int(lo), int(hi), int(default)))
    return rows


def emit(name: str, doc: str, rows: list[tuple[int, str, int, int, int]]) -> str:
    out = [f"/// {doc}", "#[rustfmt::skip]", f"pub static {name}: &[ParamSpec] = &["]
    for pid, pname, lo, hi, default in rows:
        out.append(f'    ParamSpec {{ id: {pid}, name: "{pname}", min: {lo}, max: {hi}, default: {default} }},')
    out.append("];")
    return "\n".join(out)


def emit_engine(rows: list[tuple[str, int, str, int, int, int]]) -> str:
    out = [
        "/// Parámetros por motor (CC99 0, CC98 115..122 = E1..E8). El mismo id cambia de nombre",
        "/// según el motor: mirar `engine`.",
        "#[rustfmt::skip]",
        "pub static ENGINE: &[EngineParamSpec] = &[",
    ]
    for engine, pid, pname, lo, hi, default in rows:
        out.append(
            f'    EngineParamSpec {{ engine: "{engine}", id: {pid}, name: "{pname}", '
            f"min: {lo}, max: {hi}, default: {default} }},"
        )
    out.append("];")
    return "\n".join(out)


def main() -> int:
    text = SRC.read_text(encoding="utf-8")
    track = parse_rows(text, "## Track parameters", "## Engine parameters")
    engine = parse_engine_rows(text)
    globals_ = parse_rows(text, "## Globals", "\n\n\n")

    body = "\n\n".join([
        emit("TRACK", "Parámetros de pista (CC99 0, CC98 = id), en el canal que la pista toca.", track),
        emit_engine(engine),
        emit("GLOBAL", "Globales (CC99 1, CC98 = id), en el canal 16.", globals_),
    ])
    header = (
        "// @generated por scripts/gen_nrpn.py desde docs/NRPN.md — NO EDITAR A MANO.\n"
        "// Fuente: docs/NRPN.md de JIANT (generado a su vez desde firmware/src/params.c).\n\n"
        "use crate::nrpn::{EngineParamSpec, ParamSpec};\n\n"
    )
    new = header + body + "\n"

    if "--check" in sys.argv:
        current = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        if current != new:
            print(f"{OUT} está desactualizado: corré scripts/gen_nrpn.py", file=sys.stderr)
            return 1
        print(f"ok: {len(track) + len(engine) + len(globals_)} parámetros, al día")
        return 0

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(new, encoding="utf-8")
    print(f"{OUT.name}: track={len(track)} engine={len(engine)} global={len(globals_)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

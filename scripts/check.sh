#!/usr/bin/env bash
# La puerta: lo mismo que "definición de terminado". Sin aparato conectado tiene que dar verde.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --quiet
python3 scripts/gen_nrpn.py --check
cargo build --profile fast -p fm1ctl --quiet

# Humo: la codificación verificada contra el mapa, sin abrir ningún puerto.
cli=./target/fast/fm1ctl
$cli bpm 140 --dry       | grep "F0 7D 46 4C 03 01 00 0C 41 F7" >/dev/null # editor: 140 + 8192
$cli section C --dry     | grep "CF 02" >/dev/null
$cli project 3 D --dry   | grep "CF 12" >/dev/null
$cli param track 2 FILTER 0 --dry | grep "B1 06 40" >/dev/null
$cli clock usb --dry      | grep "F0 7D 46 4C 03 01 02 01 40 F7" >/dev/null
$cli params global       | grep "BPM" >/dev/null

# Ruta de instalación: la misma familia es directa, de fork a fork pasa por el oficial, y en modo update
# no se escribe nada. Sin aparato: todo con --from.
$cli route jiant-par12 --from FM-1_016      | grep "1 paso" >/dev/null
$cli route jiant-par12 --from FM-1_9010202  | grep "2 pasos" >/dev/null
if $cli route jiant-par12 --from ota-FM-1_921 >/dev/null 2>&1; then exit 1; fi

# Humo del setlist: plantilla, guardado atómico y lectura con bytes, sin puerto.
tmp=$(mktemp -t fm1lab-setlist)
$cli setlist init "$tmp"          | grep "4 pasos" >/dev/null
$cli setlist "$tmp" --dry         | grep "CF 00" >/dev/null
$cli setlist "$tmp" --dry         | grep "CF 03" >/dev/null
rm -f "$tmp" "$tmp.tmp" "$tmp.bak"

echo "ok: fmt + clippy + tests + mapa NRPN + humo de bytes"

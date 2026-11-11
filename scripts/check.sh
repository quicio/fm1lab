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
$cli bpm 140 --dry       | grep "BF 06 40" >/dev/null # 140 BPM escalado a 14 bits = 8192
$cli section C --dry     | grep "CF 02" >/dev/null
$cli project 3 D --dry   | grep "CF 12" >/dev/null
$cli param track 2 FILTER 0 --dry | grep "B1 06 40" >/dev/null
$cli params global       | grep "BPM" >/dev/null

echo "ok: fmt + clippy + tests + mapa NRPN + humo de bytes"

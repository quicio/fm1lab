#!/usr/bin/env bash
# Convierte assets/icon.png en assets/fm1lab.icns para el bundle de macOS.
#
# Uso:
#   scripts/make_icns.sh [origen.png] [salida.icns]
set -euo pipefail

cd "$(dirname "$0")/.."
SRC="${1:-assets/icon.png}"
OUT="${2:-assets/fm1lab.icns}"

if [ ! -f "$SRC" ]; then
  echo "falta el PNG de origen: $SRC" >&2
  echo "generá el logo en assets/icon.png y volvé a correr" >&2
  exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
iconset="$work/FM1Lab.iconset"
mkdir -p "$iconset"

# La lista es la que espera `iconutil`: cada tamaño base y su @2x.
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$SRC" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$SRC" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done

iconutil -c icns "$iconset" -o "$OUT"
echo "icns -> $OUT"

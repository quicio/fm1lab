#!/usr/bin/env bash
# Arma un FM1Lab.app de macOS alrededor de un binario ya compilado.
#
# Uso:
#   scripts/make_app.sh [fm1-perf] [salida]
#
# Por defecto usa target/dist/fm1-perf y escribe target/FM1Lab.app. Si al lado
# del binario está `fm1ctl`, lo copia dentro del bundle para que el cask lo
# enlace como comando de terminal.
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$PWD"
BIN="${1:-$ROOT/target/dist/fm1-perf}"
OUT="${2:-$ROOT/target/FM1Lab.app}"
VERSION="${FM1LAB_VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)}"
VERSION="${VERSION:-0.1.0}"

if [ ! -x "$BIN" ]; then
  echo "falta el binario: $BIN" >&2
  echo "compilá con: cargo build --profile dist -p fm1-perf -p fm1ctl" >&2
  exit 1
fi

rm -rf "$OUT"
mkdir -p "$OUT/Contents/MacOS" "$OUT/Contents/Resources"

cp "$BIN" "$OUT/Contents/MacOS/fm1lab"
chmod 755 "$OUT/Contents/MacOS/fm1lab"

# fm1ctl viaja al lado del panel: el cask lo expone con `binary`.
CTL="$(dirname "$BIN")/fm1ctl"
if [ -x "$CTL" ]; then
  cp "$CTL" "$OUT/Contents/MacOS/fm1ctl"
  chmod 755 "$OUT/Contents/MacOS/fm1ctl"
fi

cat > "$OUT/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>FM1Lab</string>
  <key>CFBundleDisplayName</key>
  <string>FM1Lab</string>
  <key>CFBundleIdentifier</key>
  <string>com.quicio.fm1lab</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleExecutable</key>
  <string>fm1lab</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.music</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSHumanReadableCopyright</key>
  <string>© 2026 Hugo Moraga</string>
</dict>
</plist>
PLIST

chmod 644 "$OUT/Contents/Info.plist"

# Firma ad-hoc: evita que Gatekeeper trate el bundle como dañado. No es
# notarización de Apple; el cask se encarga de la cuarentena.
if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - "$OUT" >/dev/null 2>&1 || true
fi
xattr -dr com.apple.quarantine "$OUT" 2>/dev/null || true

echo "app -> $OUT (FM1Lab $VERSION)"

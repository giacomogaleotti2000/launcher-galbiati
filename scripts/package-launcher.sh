#!/usr/bin/env bash
# Prepara l'archivio del launcher privato (cnc-launcher) gia' compilato.
# Uso: scripts/package-launcher.sh <target-rust> <piattaforma> <versione>
# Crea dist/cnc-launcher-<versione>-<piattaforma>.(zip|tar.gz) e il file .sha256.
# Non entra nel manifest.json: il launcher si scarica una volta a mano dalla pagina della Release.
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET="$1"
PLATFORM="$2"
VERSION="$3"
APP="cnc-launcher"
EXE="$APP"
case "$TARGET" in *windows*) EXE="$APP.exe" ;; esac

TARGET_DIR="${CARGO_TARGET_DIR:-target}"
BIN="$TARGET_DIR/$TARGET/release/$EXE"
[ -f "$BIN" ] || BIN="$TARGET_DIR/release/$EXE"
[ -f "$BIN" ] || { echo "eseguibile non trovato: $EXE" >&2; exit 1; }

NAME="$APP-$VERSION-$PLATFORM"
STAGE="dist/stage/$NAME"
rm -rf "$STAGE"
mkdir -p "$STAGE/licenze"
cp "$BIN" "$STAGE/"
cp crates/ui_theme/assets/fonts/ATTRIBUTION.md crates/ui_theme/assets/fonts/OFL-*.txt "$STAGE/licenze/"
cat > "$STAGE/LEGGIMI.txt" <<TXT
CNC LAUNCHER $VERSION ($PLATFORM)

Avvio: $EXE
Al primo avvio chiede un token GitHub di sola lettura (guida: https://github.com/giacomogaleotti2000/launcher-galbiati/blob/main/docs/launcher.md)
e lo conserva nel portachiavi del sistema. Le app vengono installate nella cartella dati dell'utente.
Licenze dei font in licenze/.
TXT

mkdir -p dist
cd dist/stage
case "$PLATFORM" in
  windows-*)
    ARCHIVE="$NAME.zip"
    if command -v 7z >/dev/null; then 7z a -tzip -bd "../$ARCHIVE" "$NAME" >/dev/null; else zip -qr "../$ARCHIVE" "$NAME"; fi
    ;;
  *)
    ARCHIVE="$NAME.tar.gz"
    tar -czf "../$ARCHIVE" "$NAME"
    ;;
esac
cd ..
if command -v sha256sum >/dev/null; then sha256sum "$ARCHIVE" > "$ARCHIVE.sha256"; else shasum -a 256 "$ARCHIVE" > "$ARCHIVE.sha256"; fi
rm -rf stage
echo "creato dist/$ARCHIVE"

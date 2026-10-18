#!/usr/bin/env bash
# Buduje dist/Pakieciarka-<wersja>-x86_64.AppImage
# Każdy build podbija ostatnią cyfrę wersji w Cargo.toml (0.1.4 -> 0.1.5); --no-bump pomija to.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ARCH="x86_64"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
if [[ "${1:-}" != "--no-bump" ]]; then
    NEW="${VERSION%.*}.$(( ${VERSION##*.} + 1 ))"
    sed -i "0,/^version = \"$VERSION\"/s//version = \"$NEW\"/" Cargo.toml
    echo "==> wersja $VERSION -> $NEW"
    VERSION="$NEW"
fi
TOOLS="$ROOT/target/tools"
APPIMAGETOOL="$TOOLS/appimagetool-$ARCH.AppImage"
APPDIR="$ROOT/target/AppDir"
OUT="$ROOT/dist/Pakieciarka-$VERSION-$ARCH.AppImage"

echo "==> cargo build --release"
cargo build --release -p pakieciarka

if [[ ! -x "$APPIMAGETOOL" ]]; then
    echo "==> pobieram appimagetool"
    mkdir -p "$TOOLS"
    curl -fsSL -o "$APPIMAGETOOL" \
        "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$ARCH.AppImage"
    chmod +x "$APPIMAGETOOL"
fi

echo "==> składam AppDir"
rm -rf "$APPDIR"
install -Dm755 target/release/pakieciarka            "$APPDIR/usr/bin/pakieciarka"
install -Dm644 app/assets/pakieciarka.desktop        "$APPDIR/usr/share/applications/pakieciarka.desktop"
install -Dm644 app/assets/pakieciarka-256.png        "$APPDIR/usr/share/icons/hicolor/256x256/apps/pakieciarka.png"
install -Dm644 app/assets/pakieciarka.svg            "$APPDIR/usr/share/icons/hicolor/scalable/apps/pakieciarka.svg"
cp app/assets/pakieciarka.desktop "$APPDIR/pakieciarka.desktop"
cp app/assets/pakieciarka-256.png "$APPDIR/pakieciarka.png"
ln -s pakieciarka.png "$APPDIR/.DirIcon"
cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/pakieciarka" "$@"
RUN
chmod +x "$APPDIR/AppRun"

echo "==> appimagetool"
mkdir -p dist
rm -f "$OUT"
# --appimage-extract-and-run: działa także bez FUSE (np. w kontenerze)
ARCH="$ARCH" VERSION="$VERSION" "$APPIMAGETOOL" --appimage-extract-and-run --no-appstream "$APPDIR" "$OUT"

echo "==> gotowe: $OUT"

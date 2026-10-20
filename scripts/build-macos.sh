#!/usr/bin/env bash
# Buduje dist/Pakieciarka-<wersja>-macos.dmg — uruchamiać na macOS (lokalnie albo w GitHub Actions).
# Uniwersalna binarka (Apple Silicon + Intel), Pakieciarka.app z ikoną, podpis ad-hoc.
# Wersja jest brana z Cargo.toml bez podbijania (podbija się przy buildach Linuksa / build-all.sh).
set -euo pipefail

[[ "$(uname -s)" == "Darwin" ]] || { echo "Ten skrypt działa tylko na macOS." >&2; exit 1; }

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
export MACOSX_DEPLOYMENT_TARGET=11.0
TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)

echo "==> cargo build --release (${TARGETS[*]})"
rustup target add "${TARGETS[@]}" >/dev/null
for t in "${TARGETS[@]}"; do
    cargo build --release -p pakieciarka --target "$t"
done

BUILD="$ROOT/target/macos"
APP="$BUILD/Pakieciarka.app"
rm -rf "${BUILD:?}"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

echo "==> binarka uniwersalna"
lipo -create -output "$APP/Contents/MacOS/pakieciarka" \
    target/aarch64-apple-darwin/release/pakieciarka \
    target/x86_64-apple-darwin/release/pakieciarka

echo "==> ikona"
ICONSET="$BUILD/Pakieciarka.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
    sips -z $s $s app/assets/pakieciarka-1024.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
    sips -z $((s * 2)) $((s * 2)) app/assets/pakieciarka-1024.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Pakieciarka.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>                 <string>Pakieciarka</string>
    <key>CFBundleDisplayName</key>          <string>Pakieciarka</string>
    <key>CFBundleIdentifier</key>           <string>io.github.cyryllo.pakieciarka</string>
    <key>CFBundleExecutable</key>           <string>pakieciarka</string>
    <key>CFBundleIconFile</key>             <string>Pakieciarka</string>
    <key>CFBundlePackageType</key>          <string>APPL</string>
    <key>CFBundleShortVersionString</key>   <string>$VERSION</string>
    <key>CFBundleVersion</key>              <string>$VERSION</string>
    <key>CFBundleDevelopmentRegion</key>    <string>pl</string>
    <key>LSMinimumSystemVersion</key>       <string>11.0</string>
    <key>LSApplicationCategoryType</key>    <string>public.app-category.utilities</string>
    <key>NSHighResolutionCapable</key>      <true/>
    <key>NSHumanReadableCopyright</key>     <string>© 2026 Cyryl Sochacki · GNU GPL v3.0 lub nowsza</string>
</dict>
</plist>
PLIST

echo "==> podpis ad-hoc"
codesign --force --deep --sign - "$APP"
codesign --verify --verbose=1 "$APP"

echo "==> dmg"
STAGE="$BUILD/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Aplikacje"
cp LICENSE "$STAGE/LICENSE.txt"
mkdir -p dist
OUT="$ROOT/dist/Pakieciarka-$VERSION-macos.dmg"
rm -f "$OUT"
hdiutil create -volname "Pakieciarka $VERSION" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$OUT" >/dev/null
echo "==> gotowe: $OUT"

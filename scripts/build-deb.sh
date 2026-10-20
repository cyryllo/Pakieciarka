#!/usr/bin/env bash
# Buduje dist/pakieciarka_<wersja>_amd64.deb (Debian 13 / Ubuntu 24.04 i nowsze — wymaga glibc 2.39).
# Każdy build podbija ostatnią cyfrę wersji w Cargo.toml (0.1.4 -> 0.1.5); --no-bump pomija to.
# Instalacja: sudo apt install ./dist/pakieciarka_<wersja>_amd64.deb
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ARCH="amd64"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
if [[ "${1:-}" != "--no-bump" ]]; then
    NEW="${VERSION%.*}.$(( ${VERSION##*.} + 1 ))"
    sed -i "0,/^version = \"$VERSION\"/s//version = \"$NEW\"/" Cargo.toml
    echo "==> wersja $VERSION -> $NEW"
    VERSION="$NEW"
fi

echo "==> cargo build --release"
cargo build --release -p pakieciarka

PKG="$ROOT/target/deb/pakieciarka_${VERSION}_${ARCH}"
OUT="$ROOT/dist/pakieciarka_${VERSION}_${ARCH}.deb"

echo "==> składam pakiet"
rm -rf "${PKG:?}"
install -Dm755 target/release/pakieciarka            "$PKG/usr/bin/pakieciarka"
install -Dm644 app/assets/pakieciarka.desktop        "$PKG/usr/share/applications/pakieciarka.desktop"
install -Dm644 app/assets/pakieciarka-256.png        "$PKG/usr/share/icons/hicolor/256x256/apps/pakieciarka.png"
install -Dm644 app/assets/pakieciarka.svg            "$PKG/usr/share/icons/hicolor/scalable/apps/pakieciarka.svg"

DOC="$PKG/usr/share/doc/pakieciarka"
install -d "$DOC"
cat > "$DOC/copyright" <<COPY
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: Pakieciarka
Source: https://github.com/cyryllo/Pakieciarka

Files: *
Copyright: 2026 Cyryl Sochacki
License: GPL-3+
 This program is free software: you can redistribute it and/or modify
 it under the terms of the GNU General Public License as published by
 the Free Software Foundation, either version 3 of the License, or
 (at your option) any later version.
 .
 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 GNU General Public License for more details.
 .
 On Debian systems, the full text of the GNU General Public License
 version 3 can be found in /usr/share/common-licenses/GPL-3.
Comment:
 The binary embeds the Ubuntu Light font (Ubuntu Font Licence 1.0) and
 Hack, Noto Emoji fonts (SIL Open Font License 1.1) shipped with egui.
COPY
printf 'pakieciarka (%s) unstable; urgency=medium\n\n  * Wydanie %s — zob. https://github.com/cyryllo/Pakieciarka/releases\n\n -- %s  %s\n' \
    "$VERSION" "$VERSION" "Cyryl Sochacki <cyrylsochacki@gmail.com>" "$(LC_ALL=C date -R)" \
    | gzip -9n > "$DOC/changelog.Debian.gz"

install -d "$PKG/DEBIAN"
cat > "$PKG/DEBIAN/control" <<CTRL
Package: pakieciarka
Version: $VERSION
Section: electronics
Priority: optional
Architecture: $ARCH
Maintainer: Cyryl Sochacki <cyrylsochacki@gmail.com>
Installed-Size: $(du -sk "$PKG/usr" | cut -f1)
Depends: libc6 (>= 2.39), libgcc-s1, libgl1, libegl1, libxkbcommon0, libwayland-client0 | libx11-6
Recommends: libwayland-egl1, libx11-xcb1, libxcursor1, libxi6, libxrandr2, libxkbcommon-x11-0, libdbus-1-3, xdg-desktop-portal, xdg-utils
Homepage: https://github.com/cyryllo/Pakieciarka
Description: konfigurator pakietów akumulatorów 18650/21700
 Pakieciarka projektuje pakiety akumulatorów z ogniw cylindrycznych:
 kalkulator parametrów, rysunki połączeń niklem i odczepów balansera,
 dobór taśmy, druk PDF w skali 1:1 do ręcznego zgrzewania oraz generator
 G-code dla zgrzewarki CNC (FluidNC / GRBL).
CTRL
# stałe uprawnienia niezależnie od umask
find "$PKG" -type d -exec chmod 755 {} +
find "$PKG/usr/share" "$PKG/DEBIAN" -type f -exec chmod 644 {} +
(cd "$PKG" && find usr -type f -print0 | sort -z | xargs -0 md5sum > DEBIAN/md5sums)
chmod 644 "$PKG/DEBIAN/md5sums"

echo "==> dpkg-deb"
mkdir -p dist
dpkg-deb --root-owner-group -Zxz --build "$PKG" "$OUT" >/dev/null
echo "==> gotowe: $OUT"

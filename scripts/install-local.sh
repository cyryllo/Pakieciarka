#!/usr/bin/env bash
# Instaluje AppImage dla bieżącego użytkownika (menu aplikacji + ikona).
# Użycie: scripts/install-local.sh [ścieżka.AppImage]   |   scripts/install-local.sh --uninstall
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN_DIR="$HOME/.local/bin"
APP_FILE="$BIN_DIR/Pakieciarka.AppImage"
DESKTOP="$HOME/.local/share/applications/pakieciarka.desktop"
ICONS="$HOME/.local/share/icons/hicolor"

refresh() {
    command -v update-desktop-database >/dev/null && update-desktop-database -q "$HOME/.local/share/applications" || true
    command -v gtk-update-icon-cache  >/dev/null && gtk-update-icon-cache -q -t "$ICONS" || true
    command -v kbuildsycoca6          >/dev/null && kbuildsycoca6 >/dev/null 2>&1 || true
}

if [[ "${1:-}" == "--uninstall" ]]; then
    rm -f "$APP_FILE" "$DESKTOP" "$ICONS/256x256/apps/pakieciarka.png" "$ICONS/scalable/apps/pakieciarka.svg"
    refresh
    echo "Odinstalowano Pakieciarkę."
    exit 0
fi

SRC="${1:-$(ls -t "$ROOT"/dist/Pakieciarka-*-x86_64.AppImage 2>/dev/null | head -1)}"
if [[ -z "$SRC" || ! -f "$SRC" ]]; then
    echo "Brak AppImage — najpierw uruchom scripts/build-appimage.sh" >&2
    exit 1
fi

install -Dm755 "$SRC" "$APP_FILE"
install -Dm644 "$ROOT/app/assets/pakieciarka-256.png" "$ICONS/256x256/apps/pakieciarka.png"
install -Dm644 "$ROOT/app/assets/pakieciarka.svg"     "$ICONS/scalable/apps/pakieciarka.svg"
mkdir -p "$(dirname "$DESKTOP")"
sed "s|^Exec=.*|Exec=\"$APP_FILE\" %F|" "$ROOT/app/assets/pakieciarka.desktop" > "$DESKTOP"
refresh

echo "Zainstalowano: $APP_FILE"
echo "Pakieciarka jest w menu aplikacji (Programowanie / Elektronika)."

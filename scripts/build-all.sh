#!/usr/bin/env bash
# Podbija wersję raz i buduje AppImage oraz .deb z tym samym numerem.
set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
"$DIR/build-appimage.sh"
"$DIR/build-deb.sh" --no-bump

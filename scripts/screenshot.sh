#!/usr/bin/env bash
# Render the UI (browser preview mode, mock data) to PNGs with headless Chrome.
# Usage: scripts/screenshot.sh [outdir]   (expects `npm run dev` on :1420)
set -euo pipefail
OUT="${1:-screenshots}"
mkdir -p "$OUT"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
shot() {
  "$CHROME" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
    --window-size=1280,820 --virtual-time-budget=4000 --screenshot="$OUT/$1.png" "http://localhost:1420/$2" >/dev/null 2>&1
  echo "$OUT/$1.png"
}
shot home ""
shot onboarding "?onboarding=1"

#!/usr/bin/env bash
# Render the UI (browser preview mode, mock data) to PNGs with headless Chrome.
# Usage: scripts/screenshot.sh [outdir] [scene...]   (expects `npm run dev` on :1420)
set -euo pipefail
OUT="${1:-screenshots}"
shift || true
mkdir -p "$OUT"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
declare -a SCENES=("$@")
if [ ${#SCENES[@]} -eq 0 ]; then
  SCENES=(home onboarding unsorted group new move palette detail settings parked)
fi
url_for() {
  case "$1" in
    home) echo "" ;;
    onboarding) echo "?onboarding=1" ;;
    unsorted|group|settings|parked) echo "?view=$1" ;;
    new|move|palette|detail|park|restore) echo "?open=$1" ;;
    *) echo "" ;;
  esac
}
for s in "${SCENES[@]}"; do
  "$CHROME" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
    --window-size=1280,820 --virtual-time-budget=5000 --screenshot="$OUT/$s.png" \
    "http://localhost:1420/$(url_for "$s")" >/dev/null 2>&1
  echo "$OUT/$s.png"
done

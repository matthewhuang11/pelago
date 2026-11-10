#!/usr/bin/env bash
# Package an already-built app without Finder/AppleScript or desktop windows.
set -euo pipefail
APP="${1:-src-tauri/target/release/bundle/macos/Pelago.app}"
[[ -d "$APP/Contents" ]] || { echo "Build Pelago.app first: npm run tauri build -- --bundles app" >&2; exit 1; }
VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist")"
ARCH="$(lipo -archs "$APP/Contents/MacOS/pelago" | tr ' ' '-')"
OUT="${2:-src-tauri/target/release/bundle/dmg/Pelago_${VERSION}_${ARCH}.dmg}"
[[ ! -e "$OUT" ]] || { echo "Refusing to overwrite $OUT" >&2; exit 1; }
SCRATCH="${PELAGO_BUILD_SCRATCH:-${JCODE_SCRATCH_DIR:-${TMPDIR:-/tmp}}}"
STAGING="$(mktemp -d "$SCRATCH/pelago-dmg.XXXXXX")"
trap 'rm -rf "$STAGING"' EXIT
ditto "$APP" "$STAGING/Pelago.app"
ln -s /Applications "$STAGING/Applications"
mkdir -p "$(dirname "$OUT")"
hdiutil create -volname Pelago -srcfolder "$STAGING" -format UDZO "$OUT"
hdiutil verify "$OUT"
echo "Created unsigned development disk image: $OUT"

#!/bin/sh
# Package a built binary as edotex.app (menu bar app) and a DMG.
# Usage: sh scripts/build-macos-app.sh [BINARY] [DMG]
#   BINARY defaults to target/release/edotex (build with --features desktop),
#   DMG defaults to dist/edotex.dmg.
# The app icon is generated from assets/edotex_logo.png.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BINARY=${1:-$ROOT/target/release/edotex}
DMG=${2:-$ROOT/dist/edotex.dmg}
VERSION=$(grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d'"' -f2)
SHORT_VERSION=${VERSION%%-*}   # CFBundleShortVersionString allows only x.y.z

STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
APP="$STAGE/edotex.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/edotex"
sed -e "s/@VERSION@/$VERSION/" -e "s/@SHORT_VERSION@/$SHORT_VERSION/" \
    "$ROOT/deploy/macos/Info.plist" > "$APP/Contents/Info.plist"
# App icon: all iconset sizes from the logo, combined into edotex.icns.
ICONSET="$STAGE/edotex.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
    sips -z $size $size "$ROOT/assets/edotex_logo.png" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    sips -z $((size * 2)) $((size * 2)) "$ROOT/assets/edotex_logo.png" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/edotex.icns"
rm -rf "$ICONSET"
plutil -lint "$APP/Contents/Info.plist"
# Ad-hoc signature: required on Apple Silicon; not a Developer ID (Gatekeeper still warns).
codesign --force --sign - "$APP"

# DMG with the app and a link to /Applications for drag-and-drop installation.
ln -s /Applications "$STAGE/Applications"
mkdir -p "$(dirname "$DMG")"
rm -f "$DMG"
# hdiutil underestimates the size for -srcfolder; reserve the content size plus headroom.
SIZE_MB=$(( $(du -sm "$STAGE" | cut -f1) + 20 ))
hdiutil create -volname edotex -srcfolder "$STAGE" -fs HFS+ -size "${SIZE_MB}m" \
    -ov -format UDZO "$DMG"
echo "created $DMG"

#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONFIG="${1:-release}"

case "$CONFIG" in
  debug)
    PROFILE=dev
    TARGET_DIR=debug
    ;;
  release)
    PROFILE=release
    TARGET_DIR=release
    ;;
  *)
    echo "usage: $0 [debug|release]" >&2
    exit 1
    ;;
esac

MODULE="obs-plugin_noise-cancelling"
OUT="$ROOT/dist/$MODULE.plugin"
LIB="$ROOT/target/$TARGET_DIR/libobs_filter.dylib"
MODELS="$ROOT/models"

"$ROOT/scripts/fetch-models.sh" "$MODELS"

(
  cd "$ROOT"
  cargo build -p obs-filter --profile "$PROFILE"
)

rm -rf "$OUT"
mkdir -p "$OUT/Contents/MacOS" "$OUT/Contents/Resources/presets" "$OUT/Contents/Resources/models"
cp "$LIB" "$OUT/Contents/MacOS/$MODULE"
cp "$ROOT"/presets/*.toml "$OUT/Contents/Resources/presets/"
cp "$ROOT"/models/*.tar.gz "$OUT/Contents/Resources/models/"

cat > "$OUT/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>ja</string>
  <key>CFBundleDisplayName</key>
  <string>DeepFilterNet Noise Cancelling</string>
  <key>CFBundleExecutable</key>
  <string>obs-plugin_noise-cancelling</string>
  <key>CFBundleIdentifier</key>
  <string>com.naruton999.obs-plugin-noise-cancelling</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>obs-plugin_noise-cancelling</string>
  <key>CFBundlePackageType</key>
  <string>BNDL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>0.1.0</string>
  <key>CFBundleSupportedPlatforms</key>
  <array>
    <string>MacOSX</string>
  </array>
  <key>LSMinimumSystemVersion</key>
  <string>12.0</string>
</dict>
</plist>
PLIST

IDENTITY="${CODESIGN_IDENTITY:--}"
codesign --force --sign "$IDENTITY" "$OUT/Contents/MacOS/$MODULE"
codesign --force --sign "$IDENTITY" "$OUT"
echo "created $OUT"

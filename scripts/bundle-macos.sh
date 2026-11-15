#!/bin/sh
# Builds dist/DJSugar.app around a compiled binary and zips it as
# dist/djsugar-macos.zip. Usage: scripts/bundle-macos.sh BINARY [VERSION [BUILD]]
# VERSION is what About DJ Sugar and Finder show; it defaults to the version in
# Cargo.toml, the one the app itself reports, so the two always agree. BUILD
# goes in brackets after it in About (e.g. "test build abc1234"); it defaults
# to VERSION.
set -eu
binary=$1
version=${2:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}
build=${3:-$version}

app=dist/DJSugar.app/Contents
rm -rf dist/DJSugar.app
mkdir -p "$app/MacOS" "$app/Resources"
cp "$binary" "$app/MacOS/djsugar"
# The helper that runs audio plugins, built next to the app's binary.
helper="$(dirname "$binary")/djsugar-plugin-host"
if [ -f "$helper" ]; then
  cp "$helper" "$app/MacOS/djsugar-plugin-host"
else
  echo "warning: no $helper, so this build can't run audio plugins" >&2
fi
cp -R res/controllers "$app/Resources/controllers"
# The guided tour's demo tracks, once they are in the repository.
if [ -d res/demo ]; then cp -R res/demo "$app/Resources/demo"; fi

icons=dist/DJSugar.iconset
mkdir -p "$icons"
for size in 16 32 128 256 512; do
  sips -z $size $size res/macos/djsugar-1024.png --out "$icons/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) res/macos/djsugar-1024.png --out "$icons/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$icons" -o "$app/Resources/DJSugar.icns"

cat > "$app/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>DJ Sugar</string>
  <key>CFBundleDisplayName</key><string>DJ Sugar</string>
  <key>CFBundleIdentifier</key><string>io.github.winnerspiros.DJSugar</string>
  <key>CFBundleExecutable</key><string>djsugar</string>
  <key>CFBundleIconFile</key><string>DJ Sugar</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$build</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>DJ Sugar plays your microphone and other inputs through the mixer.</string>
  <key>NSLocalNetworkUsageDescription</key><string>Ableton Link shares tempo and beat with other music apps on your network.</string>
</dict>
</plist>
PLIST

# With MACOS_SIGN_IDENTITY set (e.g. "Developer ID Application"), signs for
# notarization: hardened runtime, secure timestamp and the entitlements.
# Without it, an ad-hoc signature, and macOS asks before the first launch.
# The plugin helper is signed first, on its own: it alone may load plugins
# signed by other developers (disable-library-validation); the app keeps
# the full hardened runtime.
if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
  if [ -f "$app/MacOS/djsugar-plugin-host" ]; then
    codesign --force --options runtime --timestamp \
      --entitlements res/macos/plugin-host.entitlements \
      --sign "$MACOS_SIGN_IDENTITY" "$app/MacOS/djsugar-plugin-host"
  fi
  codesign --force --options runtime --timestamp \
    --entitlements res/macos/djsugar.entitlements \
    --sign "$MACOS_SIGN_IDENTITY" dist/DJSugar.app
  codesign --verify --strict --verbose=2 dist/DJSugar.app
else
  if [ -f "$app/MacOS/djsugar-plugin-host" ]; then
    codesign --force --sign - "$app/MacOS/djsugar-plugin-host"
  fi
  codesign --force --sign - dist/DJSugar.app
fi
(cd dist && ditto -c -k --keepParent DJSugar.app djsugar-macos.zip)

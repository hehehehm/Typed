#!/bin/bash
# Build on macOS using only Rust and Apple's command-line tools.
set -euo pipefail

fail() { printf 'Error: %s\n' "$*" >&2; exit 1; }
[[ "$(uname -s)" == Darwin ]] || fail 'This script must run on macOS (Darwin), not Linux or Windows.'
command -v cargo >/dev/null 2>&1 || fail 'cargo is missing. Install Rust from https://rustup.rs/ and reopen Terminal.'
[[ $# -eq 0 ]] || fail 'Usage: build-dmg.sh (notarization is a separate manual step; see README.md).'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"
# Limit the lookup to [workspace.package]; do not read dependency versions.
VERSION="$(awk '
    /^\[workspace\.package\][[:space:]]*$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^[[:space:]]*version[[:space:]]*=/ {
        split($0, fields, "\""); print fields[2]; exit
    }
' Cargo.toml)"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][A-Za-z0-9.+-]+)?$ ]] || fail 'Cannot read a valid [workspace.package] version from Cargo.toml.'

cargo build --release
BIN="$REPO_ROOT/target/release/layout-keeper"
[[ -f "$BIN" ]] || fail "Built binary not found: $BIN. Build on macOS without overriding the target or target directory."

DIST="$REPO_ROOT/dist"
APP="$DIST/Typed.app"
mkdir -p "$DIST"
# Remove only the previously generated app bundle, not user configuration.
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp "$BIN" "$APP/Contents/MacOS/layout-keeper-bin"
chmod +x "$APP/Contents/MacOS/layout-keeper-bin"
mkdir -p "$APP/Contents/Resources"
ICONSET="$DIST/Typed.iconset"
rm -rf "$ICONSET"
mkdir -p "$ICONSET"
ICON_SRC="$REPO_ROOT/assets/typed-icon-app.png"
sips -z 16 16 "$ICON_SRC" --out "$ICONSET/icon_16x16.png" >/dev/null
sips -z 32 32 "$ICON_SRC" --out "$ICONSET/icon_16x16@2x.png" >/dev/null
sips -z 32 32 "$ICON_SRC" --out "$ICONSET/icon_32x32.png" >/dev/null
sips -z 64 64 "$ICON_SRC" --out "$ICONSET/icon_32x32@2x.png" >/dev/null
sips -z 128 128 "$ICON_SRC" --out "$ICONSET/icon_128x128.png" >/dev/null
sips -z 256 256 "$ICON_SRC" --out "$ICONSET/icon_128x128@2x.png" >/dev/null
sips -z 256 256 "$ICON_SRC" --out "$ICONSET/icon_256x256.png" >/dev/null
sips -z 512 512 "$ICON_SRC" --out "$ICONSET/icon_256x256@2x.png" >/dev/null
sips -z 512 512 "$ICON_SRC" --out "$ICONSET/icon_512x512.png" >/dev/null
sips -z 1024 1024 "$ICON_SRC" --out "$ICONSET/icon_512x512@2x.png" >/dev/null
if ! iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Typed.icns"; then
    printf 'Warning: iconutil failed; Finder may show a generic icon.\n' >&2
fi
rm -rf "$ICONSET"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Typed</string>
    <key>CFBundleDisplayName</key><string>Typed</string>
    <key>CFBundleIdentifier</key><string>app.layoutkeeper</string>
    <key>CFBundleExecutable</key><string>layout-keeper-bin</string>
    <key>CFBundleIconFile</key><string>Typed</string>
    <key>CFBundleIconName</key><string>Typed</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# Sign the nested binary first, then the bundle. An extra unsigned file in MacOS/
# (the old bash wrapper) made `codesign` fail with "code object is not signed at all".
if codesign --force --sign - "$APP/Contents/MacOS/layout-keeper-bin" \
        && codesign --force --sign - "$APP"
then
    :
else
    printf 'Warning: ad-hoc codesigning failed; the DMG will still be built, but macOS may refuse to launch the app.\n' >&2
fi

STAGING="$(mktemp -d "$DIST/.dmg-staging.XXXXXX")"
trap 'rm -rf "$STAGING"' EXIT
cp -R "$APP" "$STAGING/"
ln -s /Applications "$STAGING/Applications"
DMG="$DIST/Typed-$VERSION.dmg"
hdiutil create -volname Typed -srcfolder "$STAGING" -ov -format UDZO "$DMG"
printf 'Done: %s\n' "$DMG"

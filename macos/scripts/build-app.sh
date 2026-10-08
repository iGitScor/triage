#!/usr/bin/env bash
# Builds build/Remora.app from the Swift package and signs it ad hoc.
set -euo pipefail
cd "$(dirname "$0")/.."

CONFIG="${CONFIG:-release}"
swift build -c "$CONFIG"
BIN="$(swift build -c "$CONFIG" --show-bin-path)"

APP=build/Remora.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN/Remora" "$APP/Contents/MacOS/Remora"
cp Resources/Info.plist "$APP/Contents/Info.plist"
cp Resources/Outfit.woff2 "$APP/Contents/Resources/"
cp -R Resources/en.lproj Resources/fr.lproj "$APP/Contents/Resources/"
cp -R Resources/Logos "$APP/Contents/Resources/"
[ -f Resources/AppIcon.icns ] && cp Resources/AppIcon.icns "$APP/Contents/Resources/"

# Prefer an Apple-issued certificate: macOS remembers Keychain access per team ID, so rebuilds don't prompt.
# Otherwise the local self-signed certificate, otherwise ad hoc. REMORA_SIGN_IDENTITY overrides.
IDENTITIES="$(security find-identity -p codesigning)"
IDENTITY="${REMORA_SIGN_IDENTITY:-}"
if [ -z "$IDENTITY" ]; then
    IDENTITY="$(echo "$IDENTITIES" | sed -n 's/.*"\(Apple Development: [^"]*\)".*/\1/p' | head -1)"
fi
# The local certificate, under its current or its earlier name.
for NAME in "Remora Local Signing" "Perch Local Signing"; do
    if [ -z "$IDENTITY" ] && echo "$IDENTITIES" | grep -q "\"$NAME\""; then IDENTITY="$NAME"; fi
done
if [ -n "$IDENTITY" ]; then
    echo "Signing with $IDENTITY"
    codesign --force --sign "$IDENTITY" "$APP"
else
    echo "No signing certificate, signing ad hoc."
    codesign --force --sign - "$APP"
fi
echo "Built $APP"

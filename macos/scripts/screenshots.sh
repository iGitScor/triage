#!/usr/bin/env bash
# Captures the demo inbox (each tab, English and French, light and dark) for the website and the README.
# Uses a separate copy of the app in --demo mode: no account, nothing saved, no Keychain access.
# Windows are captured by ID, so nothing is clicked or typed. Needs Screen Recording for the terminal,
# an unlocked screen, and cwebp (brew install webp).
set -euo pipefail
cd "$(dirname "$0")/.."

OUT="${1:-../site/public/site/screens}"
WORK=.build/screenshots
APP="$WORK/Remora.app"

swift build -c release
BIN="$(swift build -c release --show-bin-path)"
rm -rf "$APP" && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$OUT"
cp "$BIN/Remora" "$APP/Contents/MacOS/"
cp Resources/Info.plist "$APP/Contents/"
cp -R Resources/Outfit.woff2 Resources/en.lproj Resources/fr.lproj Resources/Logos Resources/AppIcon.icns "$APP/Contents/Resources/"
codesign --force --sign - "$APP"

cat > "$WORK/window-id.swift" <<'EOF'
import CoreGraphics
let pid = Int(CommandLine.arguments[1])!
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
if let window = windows.first(where: { $0[kCGWindowOwnerPID as String] as? Int == pid && $0[kCGWindowLayer as String] as? Int == 0 }) {
    print(window[kCGWindowNumber as String]!)
}
EOF
swiftc -o "$WORK/window-id" "$WORK/window-id.swift"

cat > "$WORK/crop-top.swift" <<'EOF'
import CoreGraphics
import Foundation
import ImageIO
let url = URL(fileURLWithPath: CommandLine.arguments[1]) as CFURL
let top = Int(CommandLine.arguments[2])!
let source = CGImageSourceCreateWithURL(url, nil)!
let image = CGImageSourceCreateImageAtIndex(source, 0, nil)!
let cropped = image.cropping(to: CGRect(x: 0, y: top, width: image.width, height: image.height - top))!
let destination = CGImageDestinationCreateWithURL(url, "public.png" as CFString, 1, nil)!
CGImageDestinationAddImage(destination, cropped, nil)
CGImageDestinationFinalize(destination)
EOF
swiftc -o "$WORK/crop-top" "$WORK/crop-top.swift"

BINARY="$(cd "$APP/Contents/MacOS" && pwd)/Remora"
for lang in en fr; do
    for scheme in light dark; do
        for tab in myturn waiting snoozed; do
            args=(--demo --tab "$tab" -AppleLanguages "($lang)")
            [ "$scheme" = dark ] && args+=(--dark)
            open -n -g "$APP" --args "${args[@]}"
            file="$OUT/$tab.$lang.$scheme.png"
            rm -f "$file"
            for _ in 1 2 3 4 5; do
                sleep 2
                pid="$(pgrep -f "$BINARY" | head -1)"
                id="$("$WORK/window-id" "$pid")"
                [ -n "$id" ] && screencapture -x -o -l "$id" "$file" 2>/dev/null && break
            done
            kill "$pid"
            [ -f "$file" ] || { echo "✗ $file"; exit 1; }
            # Drop the window title bar: the site frames the inbox as the menu bar popover.
            "$WORK/crop-top" "$file" 64
            cwebp -quiet -q 85 "$file" -o "${file%.png}.webp"
            echo "✓ $file"
            sleep 1
        done
    done
done

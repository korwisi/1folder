#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
cargo build --lib

cd desktop
swift build

APP="1folder.app"
APP_DIR="$APP/Contents"
rm -rf "$APP"
mkdir -p "$APP_DIR/MacOS" "$APP_DIR/Resources" "$APP_DIR/Frameworks"

cp ".build/out/Products/Debug/OnefolderDesktop" "$APP_DIR/MacOS/OnefolderDesktop"
cp "../target/debug/libonefolder.dylib" "$APP_DIR/Frameworks/libonefolder.dylib"
cp "../assets/icon/onefolder.icns" "$APP_DIR/Resources/onefolder.icns"

RUST_DEPENDENCY=$(
  otool -L "$APP_DIR/MacOS/OnefolderDesktop" |
  awk '/libonefolder\.dylib/{print $1; exit}'
)
if [[ -n "$RUST_DEPENDENCY" ]]; then
  install_name_tool -change \
    "$RUST_DEPENDENCY" \
    "@rpath/libonefolder.dylib" \
    "$APP_DIR/MacOS/OnefolderDesktop"
fi
install_name_tool -add_rpath "@executable_path/../Frameworks" \
  "$APP_DIR/MacOS/OnefolderDesktop"

codesign --force --sign - "$APP_DIR/Frameworks/libonefolder.dylib"
codesign --force --sign - "$APP_DIR/MacOS/OnefolderDesktop"

cat > "$APP_DIR/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>OnefolderDesktop</string>
    <key>CFBundleIconFile</key>
    <string>onefolder.icns</string>
    <key>CFBundleIdentifier</key>
    <string>de.korwisi.onefolder.desktop</string>
    <key>CFBundleName</key>
    <string>1folder</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.6.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>14.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$APP"
codesign --verify --deep "$APP" >/dev/null 2>&1 || true

echo "Built desktop/$APP"

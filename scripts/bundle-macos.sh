#!/bin/sh
# Builds target/Download Anything.app. Needs Rust (https://rustup.rs).
set -e
cd "$(dirname "$0")/.."
cargo build --release
APP="target/Download Anything.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/download-anything "$APP/Contents/MacOS/"
cp assets/AppIcon.icns "$APP/Contents/Resources/"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Download Anything</string>
  <key>CFBundleIdentifier</key><string>com.lexicalluke.downloadanything</string>
  <key>CFBundleExecutable</key><string>download-anything</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
</dict></plist>
PLIST
xattr -cr "$APP"
codesign --force -s - "$APP"
echo "Built $APP"

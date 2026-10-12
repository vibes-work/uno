#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
TARGET="${TARGET:-$(rustc -vV | awk '/host:/ {print $2}')}"
case "$TARGET" in
  aarch64-apple-darwin|x86_64-apple-darwin) ;;
  *) echo "Expected a macOS Rust target, got $TARGET" >&2; exit 1 ;;
esac
rustup target add "$TARGET"
cargo build --release --locked --target "$TARGET" --bins
PACKAGE="dist/uno-$TARGET"
APP="$PACKAGE/UNO.app"
# Recreate staging so removed files cannot leak into a later package.
rm -rf "$PACKAGE"
mkdir -p "$APP/Contents/MacOS"
cp packaging/macos/Info.plist "$APP/Contents/Info.plist"
cp "target/$TARGET/release/uno" "$APP/Contents/MacOS/uno"
cp "target/$TARGET/release/uno-server" "$PACKAGE/uno-server"
cp README.md CONTRIBUTING.md LICENSE "$PACKAGE/"
cp -R docs "$PACKAGE/docs"
chmod +x "$APP/Contents/MacOS/uno" "$PACKAGE/uno-server"
# Local/ad-hoc signature only; this is NOT a notarized distribution signature.
codesign --force --sign - "$APP"
ditto -c -k --sequesterRsrc --keepParent "$PACKAGE" "$PACKAGE.zip"
echo "Created $PACKAGE.zip"

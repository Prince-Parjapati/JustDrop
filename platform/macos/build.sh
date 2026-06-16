#!/bin/bash
# Build JustDrop.app bundle for macOS
# Produces a self-contained .app with the Rust dylib embedded.

set -e

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(cd "$DIR/../.." && pwd)"
APP_DIR="$DIR/build/JustDrop.app"

echo "🔨 Building Rust library (release)..."
(cd "$PROJECT_ROOT" && cargo build --release)

echo "🔨 Building Swift app (release)..."
(cd "$DIR/JustDrop" && swift build -c release)

echo "📦 Packaging JustDrop.app..."
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Copy binary
cp "$DIR/JustDrop/.build/arm64-apple-macosx/release/JustDrop" "$APP_DIR/Contents/MacOS/JustDrop"

# Copy Info.plist
cp "$DIR/JustDrop/Info.plist" "$APP_DIR/Contents/Info.plist"

# Copy dylib and fix load path
DYLIB=$(find "$PROJECT_ROOT/target/release/deps" -name "libjustdrop_ffi.dylib" -maxdepth 1 | head -1)
if [ -z "$DYLIB" ]; then
    DYLIB="$PROJECT_ROOT/target/release/libjustdrop_ffi.dylib"
fi
cp "$DYLIB" "$APP_DIR/Contents/MacOS/libjustdrop_ffi.dylib"

# Fix the dylib reference to use @executable_path
install_name_tool -change "$DYLIB" "@executable_path/libjustdrop_ffi.dylib" "$APP_DIR/Contents/MacOS/JustDrop" 2>/dev/null || true
# Also try the deps path variant
install_name_tool -change "$PROJECT_ROOT/target/release/deps/libjustdrop_ffi.dylib" "@executable_path/libjustdrop_ffi.dylib" "$APP_DIR/Contents/MacOS/JustDrop" 2>/dev/null || true

# Remove quarantine
xattr -cr "$APP_DIR" 2>/dev/null || true

echo ""
echo "✅ Built: $APP_DIR"
echo "   To install: cp -R $APP_DIR /Applications/"
echo "   To run:     open $APP_DIR"

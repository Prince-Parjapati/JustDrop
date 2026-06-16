#!/bin/bash
# Build JustDrop.app bundle for macOS
# Produces a self-contained .app with the Rust dylib and Share Extension embedded.
set -e

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(cd "$DIR/../.." && pwd)"
APP_DIR="$DIR/build/JustDrop.app"
SDK=$(xcrun --show-sdk-path)

echo "🔨 Building Rust library (release)..."
(cd "$PROJECT_ROOT" && cargo build --release)

echo "🔨 Building Swift app (release)..."
(cd "$DIR/JustDrop" && swift build -c release)

echo "📦 Packaging JustDrop.app..."
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"
mkdir -p "$APP_DIR/Contents/PlugIns"

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
install_name_tool -change "$PROJECT_ROOT/target/release/deps/libjustdrop_ffi.dylib" "@executable_path/libjustdrop_ffi.dylib" "$APP_DIR/Contents/MacOS/JustDrop" 2>/dev/null || true

# --- Build Share Extension ---
echo "🔨 Building Share Extension..."
SHARE_DIR="$DIR/JustDropShare"
APPEX_DIR="$APP_DIR/Contents/PlugIns/JustDropShare.appex"
APPEX_CONTENTS="$APPEX_DIR/Contents"
mkdir -p "$APPEX_CONTENTS/MacOS"

# Compile Share Extension
swiftc \
    -sdk "$SDK" \
    -target arm64-apple-macosx13.0 \
    -O \
    -module-name JustDropShare \
    -emit-executable \
    -o "$APPEX_CONTENTS/MacOS/JustDropShare" \
    -I "$PROJECT_ROOT/target/release" \
    -L "$PROJECT_ROOT/target/release" \
    -L "$PROJECT_ROOT/target/release/deps" \
    -ljustdrop_ffi \
    -Xlinker -rpath -Xlinker "@executable_path/../../../MacOS" \
    "$SHARE_DIR/ShareViewController.swift" \
    2>&1 || {
        echo "⚠️  Share Extension build failed (may need Xcode). Skipping."
    }

if [ -f "$APPEX_CONTENTS/MacOS/JustDropShare" ]; then
    # Copy Info.plist for extension
    cp "$SHARE_DIR/Info.plist" "$APPEX_CONTENTS/Info.plist"

    # Fix dylib path in extension binary
    install_name_tool -change "$PROJECT_ROOT/target/release/deps/libjustdrop_ffi.dylib" "@executable_path/../../../MacOS/libjustdrop_ffi.dylib" "$APPEX_CONTENTS/MacOS/JustDropShare" 2>/dev/null || true
    install_name_tool -change "$DYLIB" "@executable_path/../../../MacOS/libjustdrop_ffi.dylib" "$APPEX_CONTENTS/MacOS/JustDropShare" 2>/dev/null || true

    # Ad-hoc codesign the extension
    codesign -s - --force --deep "$APPEX_DIR" 2>/dev/null || true
    echo "  ✓ Share Extension embedded"
else
    echo "  ⚠️  Share Extension skipped"
    rm -rf "$APPEX_DIR"
fi

# Ad-hoc codesign the main app
codesign -s - --force --deep "$APP_DIR" 2>/dev/null || true

# Remove quarantine
xattr -cr "$APP_DIR" 2>/dev/null || true

echo ""
echo "✅ Built: $APP_DIR"
echo "   To install: cp -R $APP_DIR /Applications/"
echo "   To run:     open $APP_DIR"

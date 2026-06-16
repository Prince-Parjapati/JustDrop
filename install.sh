#!/bin/bash
# JustDrop Installer for macOS
# Friendly installer that prompts for permissions natively

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$DIR"

echo "🚀 Installing JustDrop for macOS..."
echo ""

# 1. Find the app bundle (check multiple locations)
APP_SOURCE=""
if [ -d "$DIR/JustDrop.app" ]; then
    APP_SOURCE="$DIR/JustDrop.app"
elif [ -d "$DIR/platform/macos/build/JustDrop.app" ]; then
    APP_SOURCE="$DIR/platform/macos/build/JustDrop.app"
else
    echo "❌ JustDrop.app not found."
    echo "   If building from source, run: ./platform/macos/build.sh"
    exit 1
fi

# 2. Quit existing app
osascript -e 'quit app "JustDrop"' 2>/dev/null || true
killall JustDrop 2>/dev/null || true
sleep 1

# 3. Copy to Applications natively asking for admin if needed
echo "→ Copying JustDrop.app to /Applications..."
if cp -R "$APP_SOURCE" /Applications/ 2>/dev/null; then
    echo "  ✓ Copied successfully."
else
    echo "  ℹ️  Requesting permission to install to /Applications..."
    osascript -e "do shell script \"cp -R \\\"$APP_SOURCE\\\" /Applications/\" with administrator privileges"
fi

# 4. Remove quarantine (App was downloaded from internet)
xattr -cr /Applications/JustDrop.app 2>/dev/null || true

# 5. Register Share Extension with LaunchServices
/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister -f /Applications/JustDrop.app 2>/dev/null || true

# 6. Create Auto-Start LaunchAgent
echo "→ Configuring auto-start on login..."
mkdir -p "$HOME/Library/LaunchAgents"
cat <<EOF > "$HOME/Library/LaunchAgents/com.justdrop.daemon.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.justdrop.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/bin/open</string>
        <string>-a</string>
        <string>/Applications/JustDrop.app</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>LimitLoadToSessionType</key>
    <string>Aqua</string>
</dict>
</plist>
EOF
launchctl load "$HOME/Library/LaunchAgents/com.justdrop.daemon.plist" 2>/dev/null || true

# 7. Launch JustDrop
echo "→ Starting JustDrop..."
open /Applications/JustDrop.app

echo ""
echo "✅ JustDrop installed successfully!"
echo "Look for the ↔ icon in your menu bar (top-right corner)."
echo ""
echo "To share files: Right-click a file → Services → Send with JustDrop"
echo ""

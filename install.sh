#!/bin/bash
set -e

echo "🚀 Installing JustDrop for macOS..."
echo ""

# 1. Stop any existing daemon
launchctl unload ~/Library/LaunchAgents/com.justdrop.daemon.plist 2>/dev/null || true
killall JustDrop 2>/dev/null || true
killall justdrop-macos-daemon 2>/dev/null || true

# 2. Remove old binary-based install if present
sudo rm -f /usr/local/bin/justdrop-macos-daemon 2>/dev/null || true

# 3. Remove quarantine flag from the .app bundle
xattr -cr JustDrop.app 2>/dev/null || true

# 4. Copy the .app bundle to /Applications
echo "→ Copying JustDrop.app to /Applications..."
cp -R JustDrop.app /Applications/
echo "  ✓ Installed to /Applications/JustDrop.app"

# 5. Create ~/JustDrop folder for received files
mkdir -p ~/JustDrop
echo "  ✓ Created ~/JustDrop folder for received files"

# 6. Create LaunchAgent to auto-start on login
PLIST_PATH="$HOME/Library/LaunchAgents/com.justdrop.daemon.plist"
mkdir -p "$HOME/Library/LaunchAgents"
cat <<EOF > "$PLIST_PATH"
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
echo "  ✓ Auto-start on login configured"

# 7. Launch JustDrop now
echo ""
echo "→ Starting JustDrop..."
open /Applications/JustDrop.app

# 8. Flush pasteboard cache so the Share menu appears immediately
/System/Library/CoreServices/pbs -flush 2>/dev/null || true

echo ""
echo "✅ JustDrop installed successfully!"
echo ""
echo "Look for the ↔ icon in your menu bar (top-right corner, near Wi-Fi)."
echo "Click it to turn file sharing on or off."
echo ""
echo "If macOS shows a security warning:"
echo "  → Go to System Settings → Privacy & Security → click 'Open Anyway'"

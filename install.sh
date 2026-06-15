#!/bin/bash
echo "🚀 Installing JustDrop for macOS..."

# 1. Copy the binary to the local bin
sudo cp justdrop-macos-daemon /usr/local/bin/
sudo chmod +x /usr/local/bin/justdrop-macos-daemon

# 2. Create the auto-start profile
PLIST_PATH="$HOME/Library/LaunchAgents/com.justdrop.daemon.plist"
cat <<EOF > "$PLIST_PATH"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.justdrop.daemon</string>
    <key>ProgramArguments</key>
    <array><string>/usr/local/bin/justdrop-macos-daemon</string></array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
</dict>
</plist>
EOF

# 3. Start the daemon in the background
launchctl load "$PLIST_PATH"

echo "✅ Installed successfully! JustDrop is now running in the background."

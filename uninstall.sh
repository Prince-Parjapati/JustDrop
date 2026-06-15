#!/bin/bash
# JustDrop Uninstaller for macOS
# Completely removes JustDrop from your system.

set -e

echo "=== JustDrop Uninstaller ==="
echo ""

# 1. Quit the app
echo "→ Quitting JustDrop..."
osascript -e 'quit app "JustDrop"' 2>/dev/null || true
killall JustDrop 2>/dev/null || true
killall justdrop-macos-daemon 2>/dev/null || true

# 2. Stop and remove the LaunchAgent
echo "→ Removing auto-start..."
launchctl unload ~/Library/LaunchAgents/com.justdrop.daemon.plist 2>/dev/null || true
rm -f ~/Library/LaunchAgents/com.justdrop.daemon.plist

# 3. Remove the app bundle
echo "→ Removing JustDrop.app..."
rm -rf /Applications/JustDrop.app

# 4. Remove old binary install (if present)
sudo rm -f /usr/local/bin/justdrop-macos-daemon 2>/dev/null || true

# 5. Remove configuration and data
echo "→ Removing config and keys..."
rm -rf ~/Library/Application\ Support/justdrop
rm -rf ~/Library/Application\ Support/com.justdrop.app
rm -rf ~/.config/justdrop

# 6. Remove logs
echo "→ Removing logs..."
rm -f /tmp/justdrop.log /tmp/justdrop.err

echo ""
echo "✅ JustDrop has been completely removed from your Mac."
echo ""
echo "Note: The ~/JustDrop folder (received files) was NOT deleted."
echo "      Delete it manually if you no longer need those files:"
echo "      rm -rf ~/JustDrop"

#!/bin/bash
# JustDrop Uninstaller for macOS
# Completely removes JustDrop from your system.

set -e

echo "=== JustDrop Uninstaller ==="
echo ""

# 1. Stop the background service
echo "→ Stopping JustDrop daemon..."
launchctl unload ~/Library/LaunchAgents/com.justdrop.daemon.plist 2>/dev/null || true

# 2. Remove the LaunchAgent plist
echo "→ Removing LaunchAgent..."
rm -f ~/Library/LaunchAgents/com.justdrop.daemon.plist

# 3. Remove the binary
echo "→ Removing binary..."
sudo rm -f /usr/local/bin/justdrop-macos-daemon

# 4. Remove configuration and data
echo "→ Removing config and keys..."
rm -rf ~/Library/Application\ Support/justdrop
rm -rf ~/.config/justdrop

# 5. Remove logs
echo "→ Removing logs..."
rm -f /tmp/justdrop.log /tmp/justdrop.err

echo ""
echo "✅ JustDrop has been completely removed from your Mac."
echo ""
echo "Note: The ~/JustDrop folder (received files) was NOT deleted."
echo "      Delete it manually if you no longer need those files."

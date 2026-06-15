#!/bin/bash
# JustDrop Uninstaller for macOS
# Friendly uninstaller that prompts for permissions natively

echo "🗑️  Uninstalling JustDrop..."
echo ""

# 1. Quit the app
echo "→ Quitting JustDrop..."
osascript -e 'quit app "JustDrop"' 2>/dev/null || true
killall JustDrop 2>/dev/null || true

# 2. Stop and remove the LaunchAgent
echo "→ Removing auto-start login item..."
launchctl unload ~/Library/LaunchAgents/com.justdrop.daemon.plist 2>/dev/null || true
rm -f ~/Library/LaunchAgents/com.justdrop.daemon.plist

# 3. Remove the app bundle using native prompt if needed
echo "→ Removing JustDrop.app from /Applications..."
if rm -rf /Applications/JustDrop.app 2>/dev/null; then
    echo "  ✓ Removed app successfully."
else
    echo "  ℹ️  Requesting permission to delete from /Applications..."
    osascript -e "do shell script \"rm -rf /Applications/JustDrop.app\" with administrator privileges"
fi

# 4. Remove config and data
echo "→ Cleaning up configuration files..."
rm -rf ~/Library/Application\ Support/justdrop
rm -rf ~/Library/Application\ Support/com.justdrop.app
rm -rf ~/.config/justdrop

echo ""
echo "✅ JustDrop has been completely removed from your Mac."
echo ""
echo "Note: The ~/JustDrop folder (received files) was NOT deleted."
echo "You can delete it manually from Finder if you no longer need those files."
echo ""

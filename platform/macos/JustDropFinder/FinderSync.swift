import Cocoa
import FinderSync

/// Finder Sync Extension adding "Send with JustDrop" to the Finder context menu.
///
/// When the user right-clicks files in Finder, this extension adds a menu item
/// that opens the device picker and initiates a transfer.
class FinderSync: FISyncExtension {

    override init() {
        super.init()

        // Monitor all directories — the extension activates on any file selection
        let finderSync = FIFinderSyncController.default()

        // Watch commonly used directories
        if let home = FileManager.default.urls(
            for: .documentDirectory,
            in: .userDomainMask
        ).first?.deletingLastPathComponent() {
            finderSync.directoryURLs = [
                home,
                home.appendingPathComponent("Documents"),
                home.appendingPathComponent("Downloads"),
                home.appendingPathComponent("Desktop"),
            ]
        }

        NSLog("JustDrop Finder Extension: Initialized")
    }

    // MARK: - Context Menu

    override func menu(for menuKind: FIMenuKind) -> NSMenu {
        let menu = NSMenu(title: "")

        let sendItem = NSMenuItem(
            title: "Send with JustDrop",
            action: #selector(sendWithJustDrop(_:)),
            keyEquivalent: ""
        )
        sendItem.image = NSImage(systemSymbolName: "arrow.up.circle", accessibilityDescription: nil)
        menu.addItem(sendItem)

        return menu
    }

    @objc func sendWithJustDrop(_ sender: Any?) {
        guard let target = FIFinderSyncController.default().targetedURL(),
              let items = FIFinderSyncController.default().selectedItemURLs(),
              !items.isEmpty
        else {
            NSLog("JustDrop Finder: No files selected")
            return
        }

        let filePaths = items.map { $0.path }
        NSLog("JustDrop Finder: Sending \(filePaths.count) files")

        // Initialize and send via Rust engine
        let bridge = JustBridge.shared
        _ = bridge.initialize()
        _ = bridge.startDiscovery()

        // Brief delay for discovery
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
            let peers = bridge.getPeers()

            if peers.isEmpty {
                // Show alert
                let alert = NSAlert()
                alert.messageText = "No Devices Found"
                alert.informativeText = "Make sure the receiving device is on the same network and has JustDrop running."
                alert.alertStyle = .informational
                alert.addButton(withTitle: "OK")
                alert.runModal()
                return
            }

            // If only one peer, send directly
            if peers.count == 1, let peerId = peers[0]["id"] as? String {
                _ = bridge.sendFiles(peerId: peerId, filePaths: filePaths)
                return
            }

            // Show picker for multiple peers
            self.showPeerPicker(peers: peers, filePaths: filePaths)
        }
    }

    private func showPeerPicker(peers: [[String: Any]], filePaths: [String]) {
        let alert = NSAlert()
        alert.messageText = "Send to..."
        alert.informativeText = "Select a device:"

        let popup = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 250, height: 28))
        for peer in peers {
            let name = peer["name"] as? String ?? "Unknown"
            let platform = peer["platform"] as? String ?? ""
            popup.addItem(withTitle: "\(name) (\(platform))")
        }

        alert.accessoryView = popup
        alert.addButton(withTitle: "Send")
        alert.addButton(withTitle: "Cancel")

        if alert.runModal() == .alertFirstButtonReturn {
            let idx = popup.indexOfSelectedItem
            if idx >= 0 && idx < peers.count,
               let peerId = peers[idx]["id"] as? String {
                _ = JustBridge.shared.sendFiles(peerId: peerId, filePaths: filePaths)
            }
        }
    }
}

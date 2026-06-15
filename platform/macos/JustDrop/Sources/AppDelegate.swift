import Cocoa

/// AppDelegate — JustDrop runs as a menu bar agent (no dock icon).
///
/// On launch, installs a status bar icon that lets the user toggle
/// the transfer engine on and off, like AirDrop in Control Center.
/// Also registers an NSService so JustDrop appears in the Share menu.
class AppDelegate: NSObject, NSApplicationDelegate {

    private let statusBarController = StatusBarController()

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Hide from Dock — menu bar only
        NSApp.setActivationPolicy(.accessory)

        // Set up notifications (accept/reject, progress, completion)
        TransferNotificationManager.shared.setup()

        // Create ~/JustDrop folder for received files
        let justDropDir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("JustDrop")
        try? FileManager.default.createDirectory(
            at: justDropDir,
            withIntermediateDirectories: true
        )

        // Install the menu bar toggle
        statusBarController.setup()

        // Register this app as a Services provider
        NSApp.servicesProvider = self

        // Tell LaunchServices about our services
        NSUpdateDynamicServices()

        NSLog("JustDrop: Menu bar agent ready")
    }

    func applicationWillTerminate(_ notification: Notification) {
        JustBridge.shared.shutdown()
        NSLog("JustDrop: Shutdown complete")
    }

    // MARK: - macOS Services handler

    /// Called when user selects "Send with JustDrop" from Services / Share menu.
    @objc func sendWithJustDrop(
        _ pboard: NSPasteboard,
        userData: String,
        error: AutoreleasingUnsafeMutablePointer<NSString?>
    ) {
        NSLog("JustDrop Service: Invoked with pasteboard types: %@",
              pboard.types?.map { $0.rawValue }.joined(separator: ", ") ?? "none")

        var filePaths: [String] = []

        // Try to read file URLs from the pasteboard
        if let urls = pboard.readObjects(forClasses: [NSURL.self],
                                         options: [.urlReadingFileURLsOnly: true]) as? [URL] {
            filePaths = urls.map { $0.path }
        }

        // Fallback: try filenames
        if filePaths.isEmpty,
           let fileNames = pboard.propertyList(
               forType: NSPasteboard.PasteboardType("NSFilenamesPboardType")
           ) as? [String] {
            filePaths = fileNames
        }

        guard !filePaths.isEmpty else {
            NSLog("JustDrop Service: No files found on pasteboard")
            error.pointee = "No files selected" as NSString
            return
        }

        NSLog("JustDrop Service: %d file(s) to send", filePaths.count)

        // Ensure engine is running
        let bridge = JustBridge.shared
        _ = bridge.initialize()
        _ = bridge.startDiscovery()

        // Show peer picker after brief discovery period
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
            let peers = bridge.getPeers()

            if peers.isEmpty {
                let alert = NSAlert()
                alert.messageText = "No Devices Found"
                alert.informativeText = "Make sure the receiving device is on the same network and has JustDrop enabled."
                alert.alertStyle = .informational
                alert.addButton(withTitle: "OK")
                alert.runModal()
                return
            }

            // Single peer — send directly
            if peers.count == 1, let peerId = peers[0]["id"] as? String {
                let name = peers[0]["name"] as? String ?? peerId
                _ = bridge.sendFiles(peerId: peerId, filePaths: filePaths)
                NSLog("JustDrop Service: Sending to %@", name)
                return
            }

            // Multiple peers — show picker
            let alert = NSAlert()
            alert.messageText = "Send with JustDrop"
            alert.informativeText = "Select a device (\(filePaths.count) file(s)):"

            let popup = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 280, height: 28))
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
                    _ = bridge.sendFiles(peerId: peerId, filePaths: filePaths)
                }
            }
        }
    }
}

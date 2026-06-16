import Cocoa

/// AppDelegate for macOS-specific lifecycle (NSApplication delegate).
/// Handles drag-and-drop, Share menu service, and system events.
class AppDelegate: NSObject, NSApplicationDelegate {

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Register this object as the NSServices provider so the Share menu works.
        NSApp.servicesProvider = self
        // Force macOS to re-read the NSServices entries from our Info.plist.
        NSUpdateDynamicServices()
    }

    func applicationWillTerminate(_ notification: Notification) {
        justdrop_shutdown()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return false // Keep running in menu bar
    }

    // MARK: - NSServices handler

    /// Called by macOS when the user selects "Send with JustDrop" from the Share/Services menu.
    /// The selector name must match `NSMessage` in Info.plist: "sendWithJustDrop".
    @objc func sendWithJustDrop(
        _ pboard: NSPasteboard,
        userData: String,
        error errorPointer: AutoreleasingUnsafeMutablePointer<NSString>
    ) {
        // Extract file URLs from the pasteboard
        guard let items = pboard.pasteboardItems else {
            errorPointer.pointee = "No items on pasteboard" as NSString
            return
        }

        var filePaths: [String] = []
        for item in items {
            // Try public.file-url first (modern), then NSFilenamesPboardType (legacy)
            if let urlString = item.string(forType: .fileURL),
               let url = URL(string: urlString) {
                filePaths.append(url.path)
            }
        }

        // Also check for NSFilenamesPboardType (array of paths)
        if filePaths.isEmpty,
           let paths = pboard.propertyList(forType: NSPasteboard.PasteboardType("NSFilenamesPboardType")) as? [String] {
            filePaths = paths
        }

        guard !filePaths.isEmpty else {
            errorPointer.pointee = "No files found" as NSString
            return
        }

        // Post notification for the SwiftUI layer to pick up and show a peer picker
        NotificationCenter.default.post(
            name: .justDropFilesReceived,
            object: nil,
            userInfo: ["files": filePaths]
        )
    }

    /// Handle files dragged onto the dock icon
    func application(_ sender: NSApplication, openFiles filenames: [String]) {
        guard !filenames.isEmpty else { return }
        NotificationCenter.default.post(
            name: .justDropFilesReceived,
            object: nil,
            userInfo: ["files": filenames]
        )
    }
}

extension Notification.Name {
    static let justDropFilesReceived = Notification.Name("justDropFilesReceived")
    static let justDropIncomingTransfer = Notification.Name("justDropIncomingTransfer")
}

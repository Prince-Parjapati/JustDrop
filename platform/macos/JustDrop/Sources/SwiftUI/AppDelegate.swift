import Cocoa

/// AppDelegate for macOS-specific lifecycle (NSApplication delegate).
/// Handles drag-and-drop, share extension coordination, and system events.
class AppDelegate: NSObject, NSApplicationDelegate {

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Register for file drag-and-drop on the dock icon
        NSApp.registerForRemoteNotifications()
    }

    func applicationWillTerminate(_ notification: Notification) {
        // Shut down the Rust engine
        justdrop_shutdown()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return false // Keep running in menu bar
    }

    /// Handle files dragged onto the dock icon
    func application(_ sender: NSApplication, openFiles filenames: [String]) {
        guard !filenames.isEmpty else { return }
        // Post notification for the SwiftUI layer to pick up
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

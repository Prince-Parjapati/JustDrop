import Cocoa

/// AppDelegate — JustDrop runs as a menu bar agent (no dock icon).
///
/// On launch, installs a status bar icon that lets the user toggle
/// the transfer engine on and off, like AirDrop in Control Center.
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

        // Install the LaunchAgent if not already installed
        installLaunchAgent()

        NSLog("JustDrop: Menu bar agent ready")
    }

    func applicationWillTerminate(_ notification: Notification) {
        JustBridge.shared.shutdown()
        NSLog("JustDrop: Shutdown complete")
    }

    /// Install the LaunchAgent plist for auto-start on login.
    private func installLaunchAgent() {
        let launchAgentsDir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/LaunchAgents")

        let plistName = "com.justdrop.daemon.plist"
        let destPath = launchAgentsDir.appendingPathComponent(plistName)

        // Skip if already installed
        if FileManager.default.fileExists(atPath: destPath.path) {
            return
        }

        // Create LaunchAgents directory if needed
        try? FileManager.default.createDirectory(
            at: launchAgentsDir,
            withIntermediateDirectories: true
        )

        guard let executablePath = Bundle.main.executablePath else { return }

        let plistContent = """
        <?xml version="1.0" encoding="UTF-8"?>
        <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
          "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
        <plist version="1.0">
        <dict>
            <key>Label</key>
            <string>com.justdrop.daemon</string>
            <key>ProgramArguments</key>
            <array>
                <string>\(executablePath)</string>
            </array>
            <key>RunAtLoad</key>
            <true/>
            <key>KeepAlive</key>
            <true/>
            <key>StandardOutPath</key>
            <string>/tmp/justdrop.log</string>
            <key>StandardErrorPath</key>
            <string>/tmp/justdrop.err</string>
        </dict>
        </plist>
        """

        try? plistContent.write(to: destPath, atomically: true, encoding: .utf8)
        NSLog("JustDrop: LaunchAgent installed at \(destPath.path)")
    }
}

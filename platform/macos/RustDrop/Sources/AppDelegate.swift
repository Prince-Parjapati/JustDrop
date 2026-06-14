import Cocoa

/// Minimal AppDelegate — RustDrop runs as a background agent (no dock icon).
///
/// On launch, initializes the Rust engine and starts discovery.
/// The app exits immediately after setup; the daemon process runs separately.
@main
class AppDelegate: NSObject, NSApplicationDelegate {

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Hide from Dock
        NSApp.setActivationPolicy(.accessory)

        // Initialize Rust engine
        let bridge = RustBridge.shared
        guard bridge.initialize() else {
            NSLog("RustDrop: Failed to initialize Rust engine")
            NSApp.terminate(nil)
            return
        }

        guard bridge.startDiscovery() else {
            NSLog("RustDrop: Failed to start discovery")
            NSApp.terminate(nil)
            return
        }

        if let fp = bridge.getFingerprint() {
            NSLog("RustDrop: Ready. Fingerprint: \(fp)")
        }

        // Install the LaunchAgent if not already installed
        installLaunchAgent()

        NSLog("RustDrop: Background agent running")
    }

    func applicationWillTerminate(_ notification: Notification) {
        RustBridge.shared.shutdown()
        NSLog("RustDrop: Shutdown complete")
    }

    /// Install the LaunchAgent plist for auto-start on login.
    private func installLaunchAgent() {
        let launchAgentsDir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/LaunchAgents")

        let plistName = "com.rustdrop.daemon.plist"
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
            <string>com.rustdrop.daemon</string>
            <key>ProgramArguments</key>
            <array>
                <string>\(executablePath)</string>
            </array>
            <key>RunAtLoad</key>
            <true/>
            <key>KeepAlive</key>
            <true/>
            <key>StandardOutPath</key>
            <string>/tmp/rustdrop.log</string>
            <key>StandardErrorPath</key>
            <string>/tmp/rustdrop.err</string>
        </dict>
        </plist>
        """

        try? plistContent.write(to: destPath, atomically: true, encoding: .utf8)
        NSLog("RustDrop: LaunchAgent installed at \(destPath.path)")
    }
}

import Cocoa

/// Menu bar status item controller for RustDrop.
///
/// Places a small icon in the macOS menu bar (top-right, next to Wi-Fi,
/// Bluetooth, etc.) that lets users toggle RustDrop on/off with a single
/// click — similar to the AirDrop toggle in Control Center.
class StatusBarController: NSObject {

    private var statusItem: NSStatusItem?
    private var isActive: Bool = false
    private let bridge = RustBridge.shared

    /// Create and install the menu bar icon.
    func setup() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)

        guard let button = statusItem?.button else { return }
        button.image = NSImage(systemSymbolName: "arrow.triangle.swap",
                               accessibilityDescription: "RustDrop")
        button.image?.size = NSSize(width: 18, height: 18)
        button.image?.isTemplate = true

        let menu = NSMenu()

        let toggleItem = NSMenuItem(title: "Turn On RustDrop",
                                    action: #selector(toggleService(_:)),
                                    keyEquivalent: "")
        toggleItem.target = self
        toggleItem.tag = 1
        menu.addItem(toggleItem)

        menu.addItem(NSMenuItem.separator())

        let statusLabel = NSMenuItem(title: "Status: Off", action: nil, keyEquivalent: "")
        statusLabel.isEnabled = false
        statusLabel.tag = 2
        menu.addItem(statusLabel)

        menu.addItem(NSMenuItem.separator())

        let quitItem = NSMenuItem(title: "Quit RustDrop",
                                  action: #selector(quitApp(_:)),
                                  keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)

        statusItem?.menu = menu
    }

    @objc private func toggleService(_ sender: NSMenuItem) {
        if isActive {
            deactivate()
        } else {
            activate()
        }
        updateMenuState()
    }

    @objc private func quitApp(_ sender: NSMenuItem) {
        if isActive {
            deactivate()
        }
        NSApp.terminate(nil)
    }

    // MARK: - Engine control

    private func activate() {
        guard bridge.initialize() else {
            NSLog("RustDrop: Failed to initialize engine")
            return
        }
        guard bridge.startDiscovery() else {
            NSLog("RustDrop: Failed to start discovery")
            return
        }
        isActive = true
        NSLog("RustDrop: Activated from menu bar")
    }

    private func deactivate() {
        bridge.shutdown()
        isActive = false
        NSLog("RustDrop: Deactivated from menu bar")
    }

    private func updateMenuState() {
        guard let menu = statusItem?.menu else { return }

        if let toggleItem = menu.item(withTag: 1) {
            toggleItem.title = isActive ? "Turn Off RustDrop" : "Turn On RustDrop"
        }

        if let statusLabel = menu.item(withTag: 2) {
            statusLabel.title = isActive ? "Status: Receiving files…" : "Status: Off"
        }

        // Swap the icon appearance to indicate active state
        if let button = statusItem?.button {
            if isActive {
                button.image = NSImage(systemSymbolName: "arrow.triangle.swap",
                                       accessibilityDescription: "RustDrop Active")
                button.appearsDisabled = false
            } else {
                button.image = NSImage(systemSymbolName: "arrow.triangle.swap",
                                       accessibilityDescription: "RustDrop Off")
                button.appearsDisabled = true
            }
        }
    }
}

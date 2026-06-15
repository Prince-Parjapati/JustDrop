import Cocoa

/// Menu bar status item controller for JustDrop.
///
/// Places a small icon in the macOS menu bar (top-right, next to Wi-Fi,
/// Bluetooth, etc.) that lets users toggle JustDrop on/off with a single
/// click — similar to the AirDrop toggle in Control Center.
@available(macOS 11.0, *)
class StatusBarController: NSObject {

    private var statusItem: NSStatusItem?
    private var isActive: Bool = false
    private let bridge = JustBridge.shared
    private var engineInitialized: Bool = false

    /// Create and install the menu bar icon.
    func setup() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)

        guard let button = statusItem?.button else { return }
        button.image = NSImage(systemSymbolName: "arrow.triangle.swap",
                               accessibilityDescription: "JustDrop")
        button.image?.size = NSSize(width: 18, height: 18)
        button.image?.isTemplate = true

        let menu = NSMenu()

        let toggleItem = NSMenuItem(title: "Turn On JustDrop",
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

        let quitItem = NSMenuItem(title: "Quit JustDrop",
                                  action: #selector(quitApp(_:)),
                                  keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)

        statusItem?.menu = menu

        // Initialize engine once at startup
        initEngine()

        // Set initial visual state (not faded)
        updateMenuState()
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

    private func initEngine() {
        if engineInitialized { return }
        let result = bridge.initialize()
        engineInitialized = result
        if result {
            NSLog("JustDrop: Engine initialized successfully")
        } else {
            NSLog("JustDrop: Engine initialization failed")
        }
    }

    private func activate() {
        // Ensure engine is initialized
        if !engineInitialized {
            initEngine()
        }
        guard engineInitialized else {
            NSLog("JustDrop: Cannot activate - engine not initialized")
            return
        }

        let discoveryOk = bridge.startDiscovery()
        if !discoveryOk {
            NSLog("JustDrop: Failed to start discovery")
            return
        }

        isActive = true
        NSLog("JustDrop: Activated — discovery started")
    }

    private func deactivate() {
        bridge.shutdown()
        engineInitialized = false
        isActive = false
        NSLog("JustDrop: Deactivated")
    }

    private func updateMenuState() {
        guard let menu = statusItem?.menu else { return }

        if let toggleItem = menu.item(withTag: 1) {
            toggleItem.title = isActive ? "Turn Off JustDrop" : "Turn On JustDrop"
        }

        if let statusLabel = menu.item(withTag: 2) {
            statusLabel.title = isActive ? "Status: Receiving files…" : "Status: Off"
        }

        // Swap the icon appearance to indicate active state
        if let button = statusItem?.button {
            if isActive {
                button.image = NSImage(systemSymbolName: "arrow.triangle.swap",
                                       accessibilityDescription: "JustDrop Active")
                button.appearsDisabled = false
            } else {
                button.image = NSImage(systemSymbolName: "arrow.triangle.swap",
                                       accessibilityDescription: "JustDrop Off")
                button.appearsDisabled = false  // Don't fade the icon when off either
            }
            button.image?.size = NSSize(width: 18, height: 18)
            button.image?.isTemplate = true
        }
    }
}

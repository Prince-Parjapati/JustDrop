import CoreWLAN
import os.log

/// CoreWLAN wrapper for joining/leaving peer hotspots on macOS.
class WifiManager {
    private let logger = Logger(subsystem: "com.justdrop", category: "WifiManager")

    /// Join a peer's hotspot using the credentials exchanged over BLE.
    func joinNetwork(ssid: String, passphrase: String) -> Bool {
        guard let iface = CWWiFiClient.shared().interface() else {
            logger.error("No WiFi interface available")
            return false
        }

        do {
            let networks = try iface.scanForNetworks(withSSID: ssid.data(using: .utf8))
            guard let network = networks.first else {
                logger.error("Network '\(ssid)' not found")
                return false
            }

            try iface.associate(to: network, password: passphrase)
            logger.info("Joined network: \(ssid)")
            return true
        } catch {
            logger.error("Failed to join network: \(error.localizedDescription)")
            return false
        }
    }

    /// Disconnect from the current network.
    func disconnect() {
        CWWiFiClient.shared().interface()?.disassociate()
        logger.info("Disconnected from WiFi")
    }

    /// Get the current SSID.
    var currentSSID: String? {
        CWWiFiClient.shared().interface()?.ssid()
    }
}

import SwiftUI
import Combine
import CoreBluetooth

/// View models and data models for the macOS SwiftUI layer.

// MARK: - Engine ViewModel

class EngineViewModel: ObservableObject {
    @Published var isRunning: Bool = false {
        didSet {
            if isRunning { start() } else { stop() }
        }
    }
    @Published var peers: [PeerModel] = []
    @Published var transfers: [TransferModel] = []
    @Published var trustedDevices: [TrustedDevice] = []
    @Published var fingerprint: String = ""
    @Published var deviceUUID: String = ""

    private var pollTimer: Timer?

    // BLE components
    private var bleCentral: BleCentral?
    private var blePeripheral: BlePeripheral?
    private var blePeers: [String: PeerModel] = [:] // keyed by BLE address

    init() {
        // Note: didSet is NOT called during init in Swift,
        // so we must call start() explicitly here.
        isRunning = true
        start()
    }

    func start() {
        let dataDir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first?.appendingPathComponent("com.justdrop").path ?? ""

        let result = dataDir.withCString { justdrop_init($0) }
        if result == 0 {
            justdrop_start_discovery()
            startBle()
            startPolling()
        }
    }

    func stop() {
        stopPolling()
        stopBle()
        justdrop_shutdown()
        peers = []
    }

    func sendFiles(to peerId: String) {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = true
        panel.canChooseFiles = true
        panel.canChooseDirectories = false

        panel.begin { response in
            guard response == .OK else { return }
            let paths = panel.urls.map { $0.path }
            self.initiateTransfer(peerId: peerId, paths: paths)
        }
    }

    func cancelTransfer(id: String) {
        id.withCString { justdrop_cancel_transfer($0) }
    }

    func setTrust(_ deviceId: String, level: TrustLevelModel) {
        deviceId.withCString { justdrop_set_trust($0, level.rawValue) }
    }

    // MARK: - BLE

    private func startBle() {
        // Start BLE central (scanner)
        let central = BleCentral()
        central.onDeviceFound = { [weak self] address, rssi, serviceData in
            self?.handleBleDeviceFound(address: address, rssi: rssi, serviceData: serviceData)
        }
        central.onDeviceLost = { [weak self] address in
            self?.handleBleDeviceLost(address: address)
        }
        central.startScanning()
        bleCentral = central

        // Start BLE peripheral (advertiser)
        let peripheral = BlePeripheral()
        // Create advertisement payload with device info
        // The payload is parsed by the BLE scanner on the other device
        let deviceName = ProcessInfo.processInfo.hostName
        var payload = Data()
        payload.append(contentsOf: [0x4A, 0x44]) // "JD" magic bytes
        payload.append(0x01) // protocol version
        payload.append(0x01) // platform: macOS
        // Append truncated device name (max 20 bytes)
        let nameData = Data(deviceName.prefix(20).utf8)
        payload.append(UInt8(nameData.count))
        payload.append(nameData)
        peripheral.startAdvertising(payload: payload)
        blePeripheral = peripheral
    }

    private func stopBle() {
        bleCentral?.stopScanning()
        bleCentral = nil
        blePeripheral?.stopAdvertising()
        blePeripheral = nil
        blePeers.removeAll()
    }

    private func handleBleDeviceFound(address: String, rssi: Int, serviceData: Data?) {
        var name = "Unknown Device"
        var platform = "Unknown"

        // Parse JustDrop advertisement payload
        if let data = serviceData, data.count >= 4,
           data[0] == 0x4A, data[1] == 0x44 { // "JD" magic
            // data[2] = protocol version
            let platformByte = data[3]
            switch platformByte {
            case 0x01: platform = "MacOS"
            case 0x02: platform = "Android"
            case 0x03: platform = "Windows"
            case 0x04: platform = "Linux"
            default: break
            }

            // Parse device name
            if data.count > 4 {
                let nameLen = Int(data[4])
                if data.count >= 5 + nameLen {
                    name = String(data: data[5..<(5 + nameLen)], encoding: .utf8) ?? name
                }
            }
        }

        let peer = PeerModel(
            id: "ble-\(address)",
            name: name,
            platform: platform,
            address: "BLE",
            presence: .available,
            trust: .unknown,
            rssi: rssi
        )

        DispatchQueue.main.async {
            self.blePeers[address] = peer
        }
    }

    private func handleBleDeviceLost(address: String) {
        DispatchQueue.main.async {
            self.blePeers.removeValue(forKey: address)
        }
    }

    // MARK: - Polling

    private func startPolling() {
        pollTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            self?.refreshPeers()
        }
    }

    private func stopPolling() {
        pollTimer?.invalidate()
        pollTimer = nil
    }

    private func refreshPeers() {
        // Get mDNS peers from Rust
        var mdnsPeers: [PeerModel] = []
        if let json = justdrop_get_peers() {
            let str = String(cString: json)
            justdrop_free_string(json)
            if let data = str.data(using: .utf8),
               let arr = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] {
                mdnsPeers = arr.compactMap { dict in
                    guard let id = dict["id"] as? String,
                          let name = dict["name"] as? String else { return nil }
                    return PeerModel(
                        id: id,
                        name: name,
                        platform: dict["platform"] as? String ?? "Unknown",
                        address: dict["addr"] as? String,
                        presence: .available,
                        trust: .unknown,
                        rssi: dict["rssi"] as? Int
                    )
                }
            }
        }

        // Merge BLE peers with mDNS peers (deduplicate by name)
        let mdnsNames = Set(mdnsPeers.map { $0.name })
        let uniqueBlePeers = blePeers.values.filter { !mdnsNames.contains($0.name) }

        DispatchQueue.main.async {
            self.peers = mdnsPeers + uniqueBlePeers
        }
    }

    private func initiateTransfer(peerId: String, paths: [String]) {
        guard let jsonData = try? JSONSerialization.data(withJSONObject: paths),
              let jsonStr = String(data: jsonData, encoding: .utf8) else { return }
        peerId.withCString { peerPtr in
            jsonStr.withCString { pathsPtr in
                justdrop_send_files(peerPtr, pathsPtr)
            }
        }
    }
}

// MARK: - FFI Declarations

// These map to the C-ABI functions exported by justdrop-ffi.
// @_silgen_name does NOT auto-bridge types — must use raw C types.
@_silgen_name("justdrop_init")
func justdrop_init(_ dataDir: UnsafePointer<CChar>?) -> Int32

@_silgen_name("justdrop_shutdown")
func justdrop_shutdown() -> Int32

@_silgen_name("justdrop_start_discovery")
func justdrop_start_discovery() -> Int32

@_silgen_name("justdrop_get_peers")
func justdrop_get_peers() -> UnsafeMutablePointer<CChar>?

@_silgen_name("justdrop_send_files")
func justdrop_send_files(_ peerId: UnsafePointer<CChar>?, _ pathsJson: UnsafePointer<CChar>?) -> Int32

@_silgen_name("justdrop_cancel_transfer")
func justdrop_cancel_transfer(_ transferId: UnsafePointer<CChar>?) -> Int32

@_silgen_name("justdrop_set_trust")
func justdrop_set_trust(_ deviceId: UnsafePointer<CChar>?, _ level: Int32) -> Int32

@_silgen_name("justdrop_free_string")
func justdrop_free_string(_ ptr: UnsafeMutablePointer<CChar>?)

// MARK: - Models

struct PeerModel: Identifiable {
    let id: String
    let name: String
    let platform: String
    let address: String?
    let presence: PresenceModel
    let trust: TrustLevelModel
    let rssi: Int?

    var platformIcon: String {
        switch platform {
        case "MacOS": return "laptopcomputer"
        case "Android": return "candybarphone"
        case "Windows": return "desktopcomputer"
        case "Linux": return "terminal"
        default: return "desktopcomputer"
        }
    }

    var presenceColor: Color {
        switch presence {
        case .available: return .green
        case .busy: return .red
        case .receiving: return .yellow
        case .idle: return .gray
        case .invisible: return .clear
        }
    }

    var trustBadge: String? {
        switch trust {
        case .favorite: return "⭐"
        case .trusted: return "✓"
        case .blocked: return "🚫"
        case .unknown: return nil
        }
    }

    var subtitle: String {
        var parts = [platform]
        if let addr = address { parts.append(addr) }
        return parts.joined(separator: " • ")
    }

    var discoveryMethod: String {
        if id.hasPrefix("ble-") { return "BLE" }
        return "Wi-Fi"
    }
}

enum PresenceModel {
    case idle, available, receiving, busy, invisible
}

enum TrustLevelModel: Int32 {
    case unknown = 0
    case trusted = 1
    case favorite = 2
    case blocked = 3

    var label: String {
        switch self {
        case .unknown: return "Unknown"
        case .trusted: return "Trusted"
        case .favorite: return "Favorite"
        case .blocked: return "Blocked"
        }
    }
}

struct TransferModel: Identifiable {
    let id: String
    let peerName: String
    let direction: TransferDirection
    let progress: Double
    let speed: String
    let eta: String?

    enum TransferDirection {
        case send, receive
    }
}

struct TrustedDevice: Identifiable {
    let id: String
    let name: String
    let platform: String
    let fingerprint: String
    let trust: TrustLevelModel

    var platformIcon: String {
        switch platform {
        case "MacOS": return "laptopcomputer"
        case "Android": return "candybarphone"
        default: return "desktopcomputer"
        }
    }

    var trustLabel: String { trust.label }

    var trustColor: Color {
        switch trust {
        case .trusted: return .blue
        case .favorite: return .yellow
        case .blocked: return .red
        case .unknown: return .gray
        }
    }
}

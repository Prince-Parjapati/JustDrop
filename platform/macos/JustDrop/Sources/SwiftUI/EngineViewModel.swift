import SwiftUI
import Combine

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

    init() {
        // Auto-start
        isRunning = true
    }

    func start() {
        let dataDir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first?.appendingPathComponent("com.justdrop").path ?? ""

        let result = dataDir.withCString { justdrop_init($0) }
        if result == 0 {
            justdrop_start_discovery()
            startPolling()
        }
    }

    func stop() {
        stopPolling()
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
            // Call Rust FFI to initiate transfer
            self.initiateTransfer(peerId: peerId, paths: paths)
        }
    }

    func cancelTransfer(id: String) {
        id.withCString { justdrop_cancel_transfer($0) }
    }

    func setTrust(_ deviceId: String, level: TrustLevelModel) {
        deviceId.withCString { justdrop_set_trust($0, level.rawValue) }
    }

    // MARK: - Private

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
        guard let json = justdrop_get_peers() else { return }
        let str = String(cString: json)
        justdrop_free_string(json)
        guard let data = str.data(using: .utf8),
              let arr = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else { return }

        DispatchQueue.main.async {
            self.peers = arr.compactMap { dict in
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

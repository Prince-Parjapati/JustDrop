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

        let result = justdrop_init(dataDir)
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
        justdrop_cancel_transfer(id)
    }

    func setTrust(_ deviceId: String, level: TrustLevelModel) {
        justdrop_set_trust(deviceId, level.rawValue)
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
        justdrop_send_files(peerId, jsonStr)
    }
}

// MARK: - FFI Declarations

// These map to the C-ABI functions exported by justdrop-ffi.
// Will be replaced by UniFFI-generated bindings when migration completes.
@_silgen_name("justdrop_init")
func justdrop_init(_ dataDir: String) -> Int32

@_silgen_name("justdrop_shutdown")
func justdrop_shutdown() -> Int32

@_silgen_name("justdrop_start_discovery")
func justdrop_start_discovery() -> Int32

@_silgen_name("justdrop_get_peers")
func justdrop_get_peers() -> UnsafePointer<CChar>?

@_silgen_name("justdrop_send_files")
func justdrop_send_files(_ peerId: String, _ pathsJson: String) -> Int32

@_silgen_name("justdrop_cancel_transfer")
func justdrop_cancel_transfer(_ transferId: String) -> Int32

@_silgen_name("justdrop_set_trust")
func justdrop_set_trust(_ deviceId: String, _ level: Int32) -> Int32

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

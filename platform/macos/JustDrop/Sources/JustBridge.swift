import Cocoa

/// Thin Swift bridge to the JustDrop C FFI library.
///
/// Wraps the C function calls in a Swift-friendly API.
class JustBridge {

    static let shared = JustBridge()

    private var initialized = false

    private init() {}

    // MARK: - FFI Declarations

    @_silgen_name("justdrop_init")
    private static func ffi_init(_ configPath: UnsafePointer<CChar>?) -> Int32

    @_silgen_name("justdrop_start_discovery")
    private static func ffi_startDiscovery() -> Int32

    @_silgen_name("justdrop_get_peers")
    private static func ffi_getPeers() -> UnsafeMutablePointer<CChar>?

    @_silgen_name("justdrop_send_files")
    private static func ffi_sendFiles(
        _ peerId: UnsafePointer<CChar>,
        _ filePathsJson: UnsafePointer<CChar>
    ) -> Int32

    @_silgen_name("justdrop_free_string")
    private static func ffi_freeString(_ ptr: UnsafeMutablePointer<CChar>?)

    @_silgen_name("justdrop_shutdown")
    private static func ffi_shutdown() -> Int32

    @_silgen_name("justdrop_macos_set_bundle_id")
    private static func ffi_setBundleId(_ bundleId: UnsafePointer<CChar>) -> Int32

    @_silgen_name("justdrop_macos_get_fingerprint")
    private static func ffi_getFingerprint() -> UnsafeMutablePointer<CChar>?

    // MARK: - Swift API

    /// Initialize the Rust engine. Returns true if already initialized or newly initialized.
    func initialize(configPath: String? = nil) -> Bool {
        if initialized {
            NSLog("JustDrop Bridge: Already initialized")
            return true
        }

        let result: Int32
        if let path = configPath {
            result = path.withCString { JustBridge.ffi_init($0) }
        } else {
            result = JustBridge.ffi_init(nil)
        }

        if result == 0 {
            // Set bundle ID
            if let bundleId = Bundle.main.bundleIdentifier {
                bundleId.withCString { _ = JustBridge.ffi_setBundleId($0) }
            }
            initialized = true
            NSLog("JustDrop Bridge: Engine initialized OK")
        } else {
            NSLog("JustDrop Bridge: Engine init failed with code %d", result)
        }

        return result == 0
    }

    /// Start mDNS discovery.
    func startDiscovery() -> Bool {
        let result = JustBridge.ffi_startDiscovery()
        if result != 0 {
            NSLog("JustDrop Bridge: startDiscovery failed with code %d", result)
        } else {
            NSLog("JustDrop Bridge: Discovery started OK")
        }
        return result == 0
    }

    /// Get discovered peers as an array of dictionaries.
    func getPeers() -> [[String: Any]] {
        guard let cStr = JustBridge.ffi_getPeers() else { return [] }
        defer { JustBridge.ffi_freeString(cStr) }

        let json = String(cString: cStr)
        guard let data = json.data(using: .utf8),
              let peers = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
        else { return [] }

        return peers
    }

    /// Send files to a peer.
    func sendFiles(peerId: String, filePaths: [String]) -> Bool {
        guard let pathsData = try? JSONSerialization.data(
            withJSONObject: filePaths,
            options: []
        ),
        let pathsJson = String(data: pathsData, encoding: .utf8)
        else { return false }

        return peerId.withCString { peerIdPtr in
            pathsJson.withCString { pathsPtr in
                JustBridge.ffi_sendFiles(peerIdPtr, pathsPtr) == 0
            }
        }
    }

    /// Get this device's fingerprint.
    func getFingerprint() -> String? {
        guard let cStr = JustBridge.ffi_getFingerprint() else { return nil }
        defer { JustBridge.ffi_freeString(cStr) }
        return String(cString: cStr)
    }

    /// Shut down the engine.
    func shutdown() {
        _ = JustBridge.ffi_shutdown()
        initialized = false
        NSLog("JustDrop Bridge: Engine shut down")
    }
}

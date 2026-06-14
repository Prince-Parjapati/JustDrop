import Cocoa

/// Thin Swift bridge to the RustDrop C FFI library.
///
/// Wraps the C function calls in a Swift-friendly API.
class RustBridge {

    static let shared = RustBridge()

    private init() {}

    // MARK: - FFI Declarations

    @_silgen_name("rustdrop_init")
    private static func ffi_init(_ configPath: UnsafePointer<CChar>?) -> Int32

    @_silgen_name("rustdrop_start_discovery")
    private static func ffi_startDiscovery() -> Int32

    @_silgen_name("rustdrop_get_peers")
    private static func ffi_getPeers() -> UnsafeMutablePointer<CChar>?

    @_silgen_name("rustdrop_send_files")
    private static func ffi_sendFiles(
        _ peerId: UnsafePointer<CChar>,
        _ filePathsJson: UnsafePointer<CChar>
    ) -> Int32

    @_silgen_name("rustdrop_free_string")
    private static func ffi_freeString(_ ptr: UnsafeMutablePointer<CChar>?)

    @_silgen_name("rustdrop_shutdown")
    private static func ffi_shutdown() -> Int32

    @_silgen_name("rustdrop_macos_set_bundle_id")
    private static func ffi_setBundleId(_ bundleId: UnsafePointer<CChar>) -> Int32

    @_silgen_name("rustdrop_macos_get_fingerprint")
    private static func ffi_getFingerprint() -> UnsafeMutablePointer<CChar>?

    // MARK: - Swift API

    /// Initialize the Rust engine.
    func initialize(configPath: String? = nil) -> Bool {
        let result: Int32
        if let path = configPath {
            result = path.withCString { RustBridge.ffi_init($0) }
        } else {
            result = RustBridge.ffi_init(nil)
        }

        if result == 0 {
            // Set bundle ID
            if let bundleId = Bundle.main.bundleIdentifier {
                bundleId.withCString { _ = RustBridge.ffi_setBundleId($0) }
            }
        }

        return result == 0
    }

    /// Start mDNS discovery.
    func startDiscovery() -> Bool {
        return RustBridge.ffi_startDiscovery() == 0
    }

    /// Get discovered peers as an array of dictionaries.
    func getPeers() -> [[String: Any]] {
        guard let cStr = RustBridge.ffi_getPeers() else { return [] }
        defer { RustBridge.ffi_freeString(cStr) }

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
                RustBridge.ffi_sendFiles(peerIdPtr, pathsPtr) == 0
            }
        }
    }

    /// Get this device's fingerprint.
    func getFingerprint() -> String? {
        guard let cStr = RustBridge.ffi_getFingerprint() else { return nil }
        defer { RustBridge.ffi_freeString(cStr) }
        return String(cString: cStr)
    }

    /// Shut down the engine.
    func shutdown() {
        _ = RustBridge.ffi_shutdown()
    }
}

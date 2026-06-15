import CoreBluetooth
import os.log

/// BLE handshake protocol implementation.
/// Coordinates the key exchange between central and peripheral roles.
struct BleHandshake {
    private static let logger = Logger(subsystem: "com.justdrop", category: "BleHandshake")

    /// Handshake request sent from central to peripheral via GATT write.
    struct Request {
        let publicKey: Data       // 32 bytes Ed25519 public key
        let ephemeralKey: Data    // 32 bytes X25519 ephemeral public key
        let nonce: Data           // 12 bytes random nonce
        let protocolVersion: UInt8

        func encode() -> Data {
            var data = Data()
            data.append(protocolVersion)
            data.append(publicKey)
            data.append(ephemeralKey)
            data.append(nonce)
            return data
        }

        static func decode(_ data: Data) -> Request? {
            // 1 + 32 + 32 + 12 = 77 bytes
            guard data.count >= 77 else { return nil }
            return Request(
                publicKey: data[1..<33],
                ephemeralKey: data[33..<65],
                nonce: data[65..<77],
                protocolVersion: data[0]
            )
        }
    }

    /// Handshake response sent from peripheral to central via GATT notify.
    struct Response {
        let publicKey: Data       // 32 bytes
        let ephemeralKey: Data    // 32 bytes
        let nonce: Data           // 12 bytes
        let transportHint: UInt8  // Available transports (QUIC port, hotspot, etc.)
        let ipAddress: Data       // 4 or 16 bytes

        func encode() -> Data {
            var data = Data()
            data.append(publicKey)
            data.append(ephemeralKey)
            data.append(nonce)
            data.append(transportHint)
            data.append(UInt8(ipAddress.count))
            data.append(ipAddress)
            return data
        }

        static func decode(_ data: Data) -> Response? {
            guard data.count >= 78 else { return nil }
            let transportHint = data[76]
            let ipLen = Int(data[77])
            guard data.count >= 78 + ipLen else { return nil }
            return Response(
                publicKey: data[0..<32],
                ephemeralKey: data[32..<64],
                nonce: data[64..<76],
                transportHint: transportHint,
                ipAddress: data[78..<(78 + ipLen)]
            )
        }
    }

    /// Process an incoming handshake request and generate a response.
    /// Called by the peripheral (GATT server) side.
    static func processRequest(_ requestData: Data, localPublicKey: Data, localEphemeralKey: Data) -> Data? {
        guard let request = Request.decode(requestData) else {
            logger.error("Failed to decode handshake request")
            return nil
        }

        logger.info("Handshake request: version=\(request.protocolVersion), pubkey=\(request.publicKey.count)B")

        // Generate nonce for response
        var responseNonce = Data(count: 12)
        _ = responseNonce.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, 12, $0.baseAddress!) }

        // Get local IP for transport hint
        let ipData = getLocalIPv4() ?? Data([127, 0, 0, 1])

        let response = Response(
            publicKey: localPublicKey,
            ephemeralKey: localEphemeralKey,
            nonce: responseNonce,
            transportHint: 0x01, // QUIC available
            ipAddress: ipData
        )

        return response.encode()
    }

    /// Get the local IPv4 address.
    private static func getLocalIPv4() -> Data? {
        var ifaddr: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&ifaddr) == 0, let firstAddr = ifaddr else { return nil }
        defer { freeifaddrs(ifaddr) }

        for ptr in sequence(first: firstAddr, next: { $0.pointee.ifa_next }) {
            let interface_ = ptr.pointee
            let addrFamily = interface_.ifa_addr.pointee.sa_family

            if addrFamily == UInt8(AF_INET) {
                let name = String(cString: interface_.ifa_name)
                if name == "en0" || name == "en1" {
                    var addr = interface_.ifa_addr.withMemoryRebound(to: sockaddr_in.self, capacity: 1) { $0.pointee }
                    let bytes = withUnsafeBytes(of: &addr.sin_addr) { Data($0) }
                    return bytes
                }
            }
        }
        return nil
    }
}

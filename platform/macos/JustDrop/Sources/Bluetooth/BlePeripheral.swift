import CoreBluetooth
import os.log

/// CoreBluetooth peripheral manager for advertising JustDrop presence.
class BlePeripheral: NSObject, ObservableObject {
    private var peripheralManager: CBPeripheralManager?
    private let serviceUUID = CBUUID(string: "7D2D7A14-E85E-4B4F-B517-2B4C7D3F1A00")
    private let handshakeCharUUID = CBUUID(string: "7D2D7A14-E85E-4B4F-B517-2B4C7D3F1A01")
    private let logger = Logger(subsystem: "com.justdrop", category: "BlePeripheral")

    @Published var isAdvertising = false
    private var advertisementPayload: Data?
    var onHandshakeReceived: ((Data) -> Data?)?

    override init() {
        super.init()
        peripheralManager = CBPeripheralManager(delegate: self, queue: .global(qos: .userInitiated))
    }

    func startAdvertising(payload: Data) {
        advertisementPayload = payload
        guard peripheralManager?.state == .poweredOn else { return }

        let service = CBMutableService(type: serviceUUID, primary: true)
        let characteristic = CBMutableCharacteristic(
            type: handshakeCharUUID,
            properties: [.read, .write, .notify],
            value: nil,
            permissions: [.readable, .writeable]
        )
        service.characteristics = [characteristic]
        peripheralManager?.add(service)

        peripheralManager?.startAdvertising([
            CBAdvertisementDataServiceUUIDsKey: [serviceUUID],
            CBAdvertisementDataLocalNameKey: "JustDrop",
            CBAdvertisementDataServiceDataKey: [serviceUUID: payload],
        ] as [String: Any])

        DispatchQueue.main.async { self.isAdvertising = true }
        logger.info("BLE advertising started")
    }

    func stopAdvertising() {
        peripheralManager?.stopAdvertising()
        peripheralManager?.removeAllServices()
        DispatchQueue.main.async { self.isAdvertising = false }
        logger.info("BLE advertising stopped")
    }
}

extension BlePeripheral: CBPeripheralManagerDelegate {
    func peripheralManagerDidUpdateState(_ peripheral: CBPeripheralManager) {
        switch peripheral.state {
        case .poweredOn:
            logger.info("Peripheral powered on")
            if let payload = advertisementPayload {
                startAdvertising(payload: payload)
            }
        case .poweredOff:
            logger.warning("Peripheral powered off")
            DispatchQueue.main.async { self.isAdvertising = false }
        default:
            break
        }
    }

    func peripheralManager(_ peripheral: CBPeripheralManager,
                           didReceiveWrite requests: [CBATTRequest]) {
        for request in requests {
            if request.characteristic.uuid == handshakeCharUUID,
               let data = request.value {
                logger.info("Handshake data received: \(data.count) bytes")
                if let response = onHandshakeReceived?(data) {
                    request.value = response
                }
                peripheral.respond(to: request, withResult: .success)
            } else {
                peripheral.respond(to: request, withResult: .requestNotSupported)
            }
        }
    }
}

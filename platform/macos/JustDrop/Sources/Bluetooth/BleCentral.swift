import CoreBluetooth
import os.log

/// CoreBluetooth central manager for discovering nearby JustDrop peripherals.
class BleCentral: NSObject, ObservableObject {
    private var centralManager: CBCentralManager?
    private let serviceUUID = CBUUID(string: "7D2D7A14-E85E-4B4F-B517-2B4C7D3F1A00")
    private let logger = Logger(subsystem: "com.justdrop", category: "BleCentral")

    @Published var discoveredPeripherals: [CBPeripheral] = []
    @Published var isScanning = false

    var onDeviceFound: ((String, Int, Data?) -> Void)?
    var onDeviceLost: ((String) -> Void)?

    override init() {
        super.init()
        centralManager = CBCentralManager(delegate: self, queue: .global(qos: .userInitiated))
    }

    func startScanning() {
        guard centralManager?.state == .poweredOn else {
            logger.warning("BLE not ready, state: \(String(describing: self.centralManager?.state.rawValue))")
            return
        }

        centralManager?.scanForPeripherals(
            withServices: [serviceUUID],
            options: [CBCentralManagerScanOptionAllowDuplicatesKey: false]
        )
        DispatchQueue.main.async { self.isScanning = true }
        logger.info("BLE scanning started")
    }

    func stopScanning() {
        centralManager?.stopScan()
        DispatchQueue.main.async {
            self.isScanning = false
            self.discoveredPeripherals.removeAll()
        }
        logger.info("BLE scanning stopped")
    }
}

extension BleCentral: CBCentralManagerDelegate {
    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        switch central.state {
        case .poweredOn:
            logger.info("BLE powered on")
            if isScanning { startScanning() }
        case .poweredOff:
            logger.warning("BLE powered off")
            DispatchQueue.main.async { self.isScanning = false }
        case .unauthorized:
            logger.error("BLE unauthorized")
        default:
            break
        }
    }

    func centralManager(_ central: CBCentralManager,
                        didDiscover peripheral: CBPeripheral,
                        advertisementData: [String: Any],
                        rssi RSSI: NSNumber) {
        let address = peripheral.identifier.uuidString
        let serviceData = (advertisementData[CBAdvertisementDataServiceDataKey] as? [CBUUID: Data])?[serviceUUID]

        DispatchQueue.main.async {
            if !self.discoveredPeripherals.contains(where: { $0.identifier == peripheral.identifier }) {
                self.discoveredPeripherals.append(peripheral)
            }
        }

        onDeviceFound?(address, RSSI.intValue, serviceData)
    }
}

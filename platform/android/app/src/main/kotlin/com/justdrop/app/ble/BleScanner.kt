package com.justdrop.app.ble

import android.bluetooth.BluetoothManager
import android.bluetooth.le.*
import android.content.Context
import android.os.ParcelUuid
import android.util.Log

/**
 * BLE scanner for discovering nearby JustDrop devices.
 */
class BleScanner(
    private val context: Context,
) {
    companion object {
        private const val TAG = "BleScanner"
    }

    interface Listener {
        fun onDeviceFound(
            address: String,
            rssi: Int,
            serviceData: ByteArray?,
        )

        fun onDeviceLost(address: String)
    }

    private var scanner: BluetoothLeScanner? = null
    private var isScanning = false
    private var listener: Listener? = null
    private val seenDevices = mutableMapOf<String, Long>()

    private val scanCallback =
        object : ScanCallback() {
            override fun onScanResult(
                callbackType: Int,
                result: ScanResult,
            ) {
                val address = result.device.address
                val rssi = result.rssi
                val data =
                    result.scanRecord
                        ?.getServiceData(ParcelUuid(BleAdvertiser.SERVICE_UUID))

                seenDevices[address] = System.currentTimeMillis()
                listener?.onDeviceFound(address, rssi, data)
            }

            override fun onScanFailed(errorCode: Int) {
                Log.e(TAG, "BLE scan failed: $errorCode")
                isScanning = false
            }
        }

    fun start(listener: Listener) {
        this.listener = listener
        val btManager = context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager
        scanner = btManager?.adapter?.bluetoothLeScanner ?: return

        val filter =
            ScanFilter
                .Builder()
                .setServiceUuid(ParcelUuid(BleAdvertiser.SERVICE_UUID))
                .build()

        val settings =
            ScanSettings
                .Builder()
                .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
                .setReportDelay(0)
                .build()

        try {
            scanner?.startScan(listOf(filter), settings, scanCallback)
            isScanning = true
            Log.i(TAG, "BLE scanning started")
        } catch (e: SecurityException) {
            Log.e(TAG, "BLE scan permission denied", e)
        }
    }

    fun stop() {
        if (isScanning) {
            try {
                scanner?.stopScan(scanCallback)
            } catch (e: SecurityException) {
                Log.w(TAG, "Stop scan permission denied", e)
            }
            isScanning = false
            seenDevices.clear()
        }
    }
}

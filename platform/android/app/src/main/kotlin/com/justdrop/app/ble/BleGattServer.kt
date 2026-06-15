package com.justdrop.app.ble

import android.bluetooth.*
import android.content.Context
import android.util.Log
import java.util.UUID

/**
 * GATT server for BLE handshake.
 * Receives handshake requests from peer centrals and responds
 * with public key + nonce for session establishment.
 */
class BleGattServer(private val context: Context) {

    companion object {
        private const val TAG = "BleGattServer"
        val SERVICE_UUID: UUID = BleAdvertiser.SERVICE_UUID
        val HANDSHAKE_CHAR_UUID: UUID = UUID.fromString("7D2D7A14-E85E-4B4F-B517-2B4C7D3F1A01")
        val RESPONSE_CHAR_UUID: UUID = UUID.fromString("7D2D7A14-E85E-4B4F-B517-2B4C7D3F1A02")
    }

    interface Listener {
        fun onHandshakeReceived(deviceAddress: String, data: ByteArray): ByteArray?
        fun onDeviceConnected(deviceAddress: String)
        fun onDeviceDisconnected(deviceAddress: String)
    }

    private var gattServer: BluetoothGattServer? = null
    private var listener: Listener? = null
    private val connectedDevices = mutableSetOf<String>()

    private val gattCallback = object : BluetoothGattServerCallback() {
        override fun onConnectionStateChange(device: BluetoothDevice, status: Int, newState: Int) {
            when (newState) {
                BluetoothProfile.STATE_CONNECTED -> {
                    connectedDevices.add(device.address)
                    listener?.onDeviceConnected(device.address)
                    Log.i(TAG, "Device connected: ${device.address}")
                }
                BluetoothProfile.STATE_DISCONNECTED -> {
                    connectedDevices.remove(device.address)
                    listener?.onDeviceDisconnected(device.address)
                    Log.i(TAG, "Device disconnected: ${device.address}")
                }
            }
        }

        override fun onCharacteristicWriteRequest(
            device: BluetoothDevice,
            requestId: Int,
            characteristic: BluetoothGattCharacteristic,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray,
        ) {
            if (characteristic.uuid == HANDSHAKE_CHAR_UUID) {
                Log.i(TAG, "Handshake write from ${device.address}: ${value.size} bytes")
                val response = listener?.onHandshakeReceived(device.address, value)

                if (response != null) {
                    // Write response to the response characteristic
                    val respChar = gattServer
                        ?.getService(SERVICE_UUID)
                        ?.getCharacteristic(RESPONSE_CHAR_UUID)
                    respChar?.value = response
                    try {
                        gattServer?.notifyCharacteristicChanged(device, respChar, false)
                    } catch (e: SecurityException) {
                        Log.e(TAG, "Notify failed", e)
                    }
                }

                if (responseNeeded) {
                    try {
                        gattServer?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, 0, null)
                    } catch (e: SecurityException) {
                        Log.e(TAG, "Send response failed", e)
                    }
                }
            }
        }

        override fun onCharacteristicReadRequest(
            device: BluetoothDevice,
            requestId: Int,
            offset: Int,
            characteristic: BluetoothGattCharacteristic,
        ) {
            if (characteristic.uuid == RESPONSE_CHAR_UUID) {
                try {
                    gattServer?.sendResponse(
                        device, requestId, BluetoothGatt.GATT_SUCCESS, 0,
                        characteristic.value ?: ByteArray(0)
                    )
                } catch (e: SecurityException) {
                    Log.e(TAG, "Read response failed", e)
                }
            }
        }
    }

    fun start(listener: Listener) {
        this.listener = listener
        val btManager = context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager ?: return

        try {
            gattServer = btManager.openGattServer(context, gattCallback)
        } catch (e: SecurityException) {
            Log.e(TAG, "GATT server permission denied", e)
            return
        }

        val service = BluetoothGattService(SERVICE_UUID, BluetoothGattService.SERVICE_TYPE_PRIMARY)

        val handshakeChar = BluetoothGattCharacteristic(
            HANDSHAKE_CHAR_UUID,
            BluetoothGattCharacteristic.PROPERTY_WRITE,
            BluetoothGattCharacteristic.PERMISSION_WRITE,
        )

        val responseChar = BluetoothGattCharacteristic(
            RESPONSE_CHAR_UUID,
            BluetoothGattCharacteristic.PROPERTY_READ or BluetoothGattCharacteristic.PROPERTY_NOTIFY,
            BluetoothGattCharacteristic.PERMISSION_READ,
        )

        service.addCharacteristic(handshakeChar)
        service.addCharacteristic(responseChar)

        try {
            gattServer?.addService(service)
        } catch (e: SecurityException) {
            Log.e(TAG, "Add service failed", e)
        }
        Log.i(TAG, "GATT server started")
    }

    fun stop() {
        try {
            gattServer?.close()
        } catch (e: SecurityException) {
            Log.w(TAG, "GATT server close failed", e)
        }
        gattServer = null
        connectedDevices.clear()
    }
}

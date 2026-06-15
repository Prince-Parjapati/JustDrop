package com.justdrop.app.ble

import android.bluetooth.*
import android.content.Context
import android.util.Log
import java.util.UUID

/**
 * GATT client for BLE handshake.
 * Connects to a peer peripheral, writes handshake data,
 * and reads the response containing session parameters.
 */
class BleGattClient(
    private val context: Context,
) {
    companion object {
        private const val TAG = "BleGattClient"
    }

    interface Listener {
        fun onHandshakeComplete(
            deviceAddress: String,
            responseData: ByteArray,
        )

        fun onHandshakeFailed(
            deviceAddress: String,
            reason: String,
        )
    }

    private var activeGatt: BluetoothGatt? = null
    private var listener: Listener? = null
    private var pendingHandshakeData: ByteArray? = null

    private val gattCallback =
        object : BluetoothGattCallback() {
            override fun onConnectionStateChange(
                gatt: BluetoothGatt,
                status: Int,
                newState: Int,
            ) {
                when (newState) {
                    BluetoothProfile.STATE_CONNECTED -> {
                        Log.i(TAG, "Connected to ${gatt.device.address}, discovering services")
                        try {
                            gatt.discoverServices()
                        } catch (e: SecurityException) {
                            Log.e(TAG, "Discover services permission denied", e)
                        }
                    }

                    BluetoothProfile.STATE_DISCONNECTED -> {
                        Log.i(TAG, "Disconnected from ${gatt.device.address}")
                        activeGatt = null
                    }
                }
            }

            override fun onServicesDiscovered(
                gatt: BluetoothGatt,
                status: Int,
            ) {
                if (status != BluetoothGatt.GATT_SUCCESS) {
                    listener?.onHandshakeFailed(gatt.device.address, "Service discovery failed: $status")
                    return
                }

                val service = gatt.getService(BleGattServer.SERVICE_UUID)
                if (service == null) {
                    listener?.onHandshakeFailed(gatt.device.address, "JustDrop service not found")
                    return
                }

                // Enable notifications on response characteristic
                val responseChar = service.getCharacteristic(BleGattServer.RESPONSE_CHAR_UUID)
                if (responseChar != null) {
                    try {
                        gatt.setCharacteristicNotification(responseChar, true)
                        val descriptor =
                            responseChar.getDescriptor(
                                UUID.fromString("00002902-0000-1000-8000-00805f9b34fb"),
                            )
                        descriptor?.value = BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
                        gatt.writeDescriptor(descriptor)
                    } catch (e: SecurityException) {
                        Log.e(TAG, "Enable notifications failed", e)
                    }
                }

                // Write handshake data
                val handshakeChar = service.getCharacteristic(BleGattServer.HANDSHAKE_CHAR_UUID)
                if (handshakeChar != null && pendingHandshakeData != null) {
                    handshakeChar.value = pendingHandshakeData
                    try {
                        gatt.writeCharacteristic(handshakeChar)
                    } catch (e: SecurityException) {
                        Log.e(TAG, "Write handshake failed", e)
                    }
                    Log.i(TAG, "Handshake data written: ${pendingHandshakeData!!.size} bytes")
                }
            }

            @Deprecated("Deprecated in API 33")
            override fun onCharacteristicChanged(
                gatt: BluetoothGatt,
                characteristic: BluetoothGattCharacteristic,
            ) {
                if (characteristic.uuid == BleGattServer.RESPONSE_CHAR_UUID) {
                    val data = characteristic.value
                    Log.i(TAG, "Handshake response: ${data.size} bytes")
                    listener?.onHandshakeComplete(gatt.device.address, data)
                    disconnect()
                }
            }
        }

    fun performHandshake(
        device: BluetoothDevice,
        data: ByteArray,
        listener: Listener,
    ) {
        this.listener = listener
        this.pendingHandshakeData = data

        try {
            activeGatt = device.connectGatt(context, false, gattCallback, BluetoothDevice.TRANSPORT_LE)
        } catch (e: SecurityException) {
            Log.e(TAG, "Connect GATT permission denied", e)
            listener.onHandshakeFailed(device.address, "Permission denied")
        }
    }

    fun disconnect() {
        try {
            activeGatt?.disconnect()
            activeGatt?.close()
        } catch (e: SecurityException) {
            Log.w(TAG, "Disconnect failed", e)
        }
        activeGatt = null
        pendingHandshakeData = null
    }
}

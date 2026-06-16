package com.justdrop.app

import android.app.*
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import com.justdrop.app.ble.BleAdvertiser
import com.justdrop.app.ble.BleScanner

/**
 * Foreground service running the JustDrop engine.
 *
 * Starts on boot and keeps the Rust transfer engine alive for
 * receiving incoming files and maintaining mDNS + BLE presence.
 */
class JustDropService :
    Service(),
    BleScanner.Listener {
    companion object {
        private const val TAG = "JustDropService"
        private const val NOTIFICATION_ID = 1
        private const val CHANNEL_ID = "justdrop_service"
        private const val CHANNEL_NAME = "JustDrop Service"
    }

    private var multicastLock: android.net.wifi.WifiManager.MulticastLock? = null
    private var bleAdvertiser: BleAdvertiser? = null
    private var bleScanner: BleScanner? = null

    override fun onCreate() {
        super.onCreate()
        Log.i(TAG, "Service created")
        JustDropApp.isServiceRunning = true
        createNotificationChannel()
        startForegroundService()

        // Acquire MulticastLock to receive mDNS packets
        val wifi =
            applicationContext.getSystemService(android.content.Context.WIFI_SERVICE) as android.net.wifi.WifiManager
        multicastLock = wifi.createMulticastLock("JustDropMulticastLock")
        multicastLock?.setReferenceCounted(true)
        multicastLock?.acquire()

        initRustEngine()
        startBle()
    }

    override fun onStartCommand(
        intent: Intent?,
        flags: Int,
        startId: Int,
    ): Int {
        return START_STICKY // Restart if killed
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        Log.i(TAG, "Service destroyed, shutting down")
        JustDropApp.isServiceRunning = false
        stopBle()
        multicastLock?.release()
        multicastLock = null
        JustBridge.shutdown()
        super.onDestroy()
    }

    // MARK: - BLE

    private fun startBle() {
        try {
            // Start BLE advertising
            val advertiser = BleAdvertiser(this)
            val deviceName = Build.MODEL
            // Build advertisement payload: magic(2) + version(1) + platform(1) + nameLen(1) + name(N)
            val nameBytes = deviceName.toByteArray(Charsets.UTF_8).take(20).toByteArray()
            val payload =
                byteArrayOf(
                    0x4A,
                    0x44, // "JD" magic
                    0x01, // protocol version
                    0x02, // platform: Android
                    nameBytes.size.toByte(),
                ) + nameBytes
            advertiser.start(payload)
            bleAdvertiser = advertiser
            Log.i(TAG, "BLE advertising started")

            // Start BLE scanning
            val scanner = BleScanner(this)
            scanner.start(this)
            bleScanner = scanner
            Log.i(TAG, "BLE scanning started")
        } catch (e: Exception) {
            Log.w(TAG, "BLE start failed (may not have permission): ${e.message}")
        }
    }

    private fun stopBle() {
        bleAdvertiser?.stop()
        bleAdvertiser = null
        bleScanner?.stop()
        bleScanner = null
    }

    // BleScanner.Listener
    override fun onDeviceFound(
        address: String,
        rssi: Int,
        serviceData: ByteArray?,
    ) {
        var name = "Unknown"
        var platform = "Unknown"
        if (serviceData != null && serviceData.size >= 4 &&
            serviceData[0] == 0x4A.toByte() && serviceData[1] == 0x44.toByte()
        ) {
            when (serviceData[3].toInt()) {
                0x01 -> platform = "MacOS"
                0x02 -> platform = "Android"
                0x03 -> platform = "Windows"
                0x04 -> platform = "Linux"
            }
            if (serviceData.size > 4) {
                val nameLen = serviceData[4].toInt() and 0xFF
                if (serviceData.size >= 5 + nameLen) {
                    name = String(serviceData, 5, nameLen, Charsets.UTF_8)
                }
            }
        }
        Log.d(TAG, "BLE device found: $name ($platform) rssi=$rssi addr=$address")
    }

    override fun onDeviceLost(address: String) {
        Log.d(TAG, "BLE device lost: $address")
    }

    // MARK: - Notification

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel =
                NotificationChannel(
                    CHANNEL_ID,
                    CHANNEL_NAME,
                    NotificationManager.IMPORTANCE_LOW,
                ).apply {
                    description = "JustDrop file transfer service"
                    setShowBadge(false)
                }
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(channel)
        }
    }

    private fun startForegroundService() {
        val notification =
            NotificationCompat
                .Builder(this, CHANNEL_ID)
                .setContentTitle("JustDrop")
                .setContentText("Ready to receive files")
                .setSmallIcon(android.R.drawable.ic_menu_share)
                .setPriority(NotificationCompat.PRIORITY_LOW)
                .setOngoing(true)
                .build()

        ServiceCompat.startForeground(
            this,
            NOTIFICATION_ID,
            notification,
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC
            } else {
                0
            },
        )
    }

    private fun initRustEngine() {
        // Received files go to /sdcard/JustDrop for easy access
        val justDropDir =
            java.io.File(
                android.os.Environment.getExternalStorageDirectory(),
                "JustDrop",
            )
        if (!justDropDir.exists()) justDropDir.mkdirs()

        JustBridge.setDownloadsDir(justDropDir.absolutePath)
        JustBridge.setDataDir(filesDir.absolutePath)

        // Initialize Rust engine
        val result = JustBridge.init(null)
        if (result != 0) {
            Log.e(TAG, "Rust engine init failed: $result")
            return
        }

        // Start discovery
        val discResult = JustBridge.startDiscovery()
        if (discResult != 0) {
            Log.e(TAG, "Discovery start failed: $discResult")
            return
        }

        Log.i(TAG, "Rust engine initialized and discovery started")
    }
}

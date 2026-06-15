package com.justdrop.app

import android.app.*
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat

/**
 * Foreground service running the JustDrop engine.
 *
 * Starts on boot and keeps the Rust transfer engine alive for
 * receiving incoming files and maintaining mDNS presence.
 */
class JustDropService : Service() {

    companion object {
        private const val TAG = "JustDropService"
        private const val NOTIFICATION_ID = 1
        private const val CHANNEL_ID = "justdrop_service"
        private const val CHANNEL_NAME = "JustDrop Service"
    }

    private var multicastLock: android.net.wifi.WifiManager.MulticastLock? = null

    override fun onCreate() {
        super.onCreate()
        Log.i(TAG, "Service created")
        JustDropApp.isServiceRunning = true
        createNotificationChannel()
        startForegroundService()

        // Acquire MulticastLock to receive mDNS packets
        val wifi = applicationContext.getSystemService(android.content.Context.WIFI_SERVICE) as android.net.wifi.WifiManager
        multicastLock = wifi.createMulticastLock("JustDropMulticastLock")
        multicastLock?.setReferenceCounted(true)
        multicastLock?.acquire()

        initRustEngine()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        return START_STICKY // Restart if killed
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        Log.i(TAG, "Service destroyed, shutting down Rust engine")
        JustDropApp.isServiceRunning = false
        multicastLock?.release()
        multicastLock = null
        JustBridge.shutdown()
        super.onDestroy()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                CHANNEL_NAME,
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "JustDrop file transfer service"
                setShowBadge(false)
            }
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(channel)
        }
    }

    private fun startForegroundService() {
        val notification = NotificationCompat.Builder(this, CHANNEL_ID)
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
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q)
                ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC
            else 0
        )
    }

    private fun initRustEngine() {
        // Received files go to /sdcard/JustDrop for easy access
        val justDropDir = java.io.File(
            android.os.Environment.getExternalStorageDirectory(), "JustDrop"
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

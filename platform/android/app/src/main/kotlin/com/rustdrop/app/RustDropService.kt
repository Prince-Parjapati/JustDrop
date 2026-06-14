package com.rustdrop.app

import android.app.*
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat

/**
 * Foreground service running the RustDrop engine.
 *
 * Starts on boot and keeps the Rust transfer engine alive for
 * receiving incoming files and maintaining mDNS presence.
 */
class RustDropService : Service() {

    companion object {
        private const val TAG = "RustDropService"
        private const val NOTIFICATION_ID = 1
        private const val CHANNEL_ID = "rustdrop_service"
        private const val CHANNEL_NAME = "RustDrop Service"
    }

    override fun onCreate() {
        super.onCreate()
        Log.i(TAG, "Service created")
        createNotificationChannel()
        startForegroundService()
        initRustEngine()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        return START_STICKY // Restart if killed
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        Log.i(TAG, "Service destroyed, shutting down Rust engine")
        RustBridge.shutdown()
        super.onDestroy()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                CHANNEL_NAME,
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "RustDrop file transfer service"
                setShowBadge(false)
            }
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(channel)
        }
    }

    private fun startForegroundService() {
        val notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("RustDrop")
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
        // Set platform-specific paths
        RustBridge.setDownloadsDir(
            getExternalFilesDir(null)?.absolutePath
                ?: filesDir.absolutePath
        )
        RustBridge.setDataDir(filesDir.absolutePath)

        // Initialize Rust engine
        val result = RustBridge.init(null)
        if (result != 0) {
            Log.e(TAG, "Rust engine init failed: $result")
            return
        }

        // Start discovery
        val discResult = RustBridge.startDiscovery()
        if (discResult != 0) {
            Log.e(TAG, "Discovery start failed: $discResult")
            return
        }

        Log.i(TAG, "Rust engine initialized and discovery started")
    }
}

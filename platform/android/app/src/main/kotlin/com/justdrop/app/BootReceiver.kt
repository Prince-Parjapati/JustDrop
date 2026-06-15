package com.justdrop.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log

/**
 * Starts the JustDrop service on device boot.
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(
        context: Context,
        intent: Intent,
    ) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED) {
            Log.i("JustDrop", "Boot completed, starting service")
            JustDropApp.isServiceRunning = true
            val serviceIntent = Intent(context, JustDropService::class.java)
            context.startForegroundService(serviceIntent)
        }
    }
}

package com.rustdrop.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log

/**
 * Starts the RustDrop service on device boot.
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED) {
            Log.i("RustDrop", "Boot completed, starting service")
            val serviceIntent = Intent(context, RustDropService::class.java)
            context.startForegroundService(serviceIntent)
        }
    }
}

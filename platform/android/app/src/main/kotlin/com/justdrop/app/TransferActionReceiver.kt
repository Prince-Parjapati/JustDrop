package com.justdrop.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log

/**
 * Receives Accept / Reject actions from the incoming transfer notification.
 *
 * When the user taps Accept or Reject on the notification, this receiver
 * forwards the decision to the Rust engine via JNI.
 */
class TransferActionReceiver : BroadcastReceiver() {
    companion object {
        private const val TAG = "TransferAction"
    }

    override fun onReceive(
        context: Context,
        intent: Intent,
    ) {
        val transferId = intent.getIntExtra("transfer_id", -1)
        if (transferId == -1) return

        when (intent.action) {
            "com.justdrop.ACTION_ACCEPT" -> {
                Log.i(TAG, "User accepted transfer $transferId")
                JustBridge.acceptTransfer(transferId.toString())
            }

            "com.justdrop.ACTION_REJECT" -> {
                Log.i(TAG, "User rejected transfer $transferId")
                JustBridge.rejectTransfer(transferId.toString())
            }
        }

        // Dismiss the notification
        val nm =
            context.getSystemService(Context.NOTIFICATION_SERVICE)
                as android.app.NotificationManager
        nm.cancel(2000 + transferId)
    }
}

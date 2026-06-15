package com.justdrop.app

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat

/**
 * Handles all JustDrop notifications:
 *
 * - Incoming transfer request (accept / reject actions)
 * - Transfer progress (ongoing, with progress bar)
 * - Transfer complete (file saved location)
 * - Transfer failed
 */
object TransferNotifications {

    private const val CHANNEL_TRANSFER = "justdrop_transfers"
    private const val CHANNEL_REQUEST = "justdrop_requests"

    private const val REQUEST_NOTIFICATION_BASE = 2000
    private const val PROGRESS_NOTIFICATION_BASE = 3000
    private const val COMPLETE_NOTIFICATION_BASE = 4000

    fun createChannels(context: Context) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val manager = context.getSystemService(NotificationManager::class.java)

            val requestChannel = NotificationChannel(
                CHANNEL_REQUEST,
                "Transfer Requests",
                NotificationManager.IMPORTANCE_HIGH
            ).apply {
                description = "Incoming file transfer requests"
                enableVibration(true)
            }

            val transferChannel = NotificationChannel(
                CHANNEL_TRANSFER,
                "Transfer Progress",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Ongoing file transfer progress"
                setShowBadge(false)
            }

            manager.createNotificationChannel(requestChannel)
            manager.createNotificationChannel(transferChannel)
        }
    }

    /**
     * Show an incoming transfer request notification with Accept / Reject buttons.
     */
    fun showIncomingRequest(
        context: Context,
        transferId: Int,
        senderName: String,
        fileCount: Int,
        totalSizeFormatted: String
    ) {
        val acceptIntent = Intent(context, TransferActionReceiver::class.java).apply {
            action = "com.justdrop.ACTION_ACCEPT"
            putExtra("transfer_id", transferId)
        }
        val rejectIntent = Intent(context, TransferActionReceiver::class.java).apply {
            action = "com.justdrop.ACTION_REJECT"
            putExtra("transfer_id", transferId)
        }

        val acceptPending = PendingIntent.getBroadcast(
            context, transferId, acceptIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val rejectPending = PendingIntent.getBroadcast(
            context, transferId + 10000, rejectIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val description = if (fileCount == 1) {
            "$senderName wants to send you a file ($totalSizeFormatted)"
        } else {
            "$senderName wants to send you $fileCount files ($totalSizeFormatted)"
        }

        val notification = NotificationCompat.Builder(context, CHANNEL_REQUEST)
            .setSmallIcon(android.R.drawable.ic_menu_share)
            .setContentTitle("Incoming Transfer")
            .setContentText(description)
            .setStyle(NotificationCompat.BigTextStyle().bigText(description))
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_SOCIAL)
            .setAutoCancel(true)
            .addAction(android.R.drawable.ic_menu_send, "Accept", acceptPending)
            .addAction(android.R.drawable.ic_menu_close_clear_cancel, "Reject", rejectPending)
            .build()

        try {
            NotificationManagerCompat.from(context)
                .notify(REQUEST_NOTIFICATION_BASE + transferId, notification)
        } catch (_: SecurityException) {
            // Notification permission not granted
        }
    }

    /**
     * Show or update transfer progress notification with a progress bar.
     */
    fun showProgress(
        context: Context,
        transferId: Int,
        fileName: String,
        percent: Int,
        speedFormatted: String
    ) {
        val notification = NotificationCompat.Builder(context, CHANNEL_TRANSFER)
            .setSmallIcon(android.R.drawable.stat_sys_download)
            .setContentTitle("Receiving: $fileName")
            .setContentText("$percent% • $speedFormatted")
            .setProgress(100, percent, false)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()

        try {
            NotificationManagerCompat.from(context)
                .notify(PROGRESS_NOTIFICATION_BASE + transferId, notification)
        } catch (_: SecurityException) { }
    }

    /**
     * Show transfer complete notification with the saved location.
     */
    fun showComplete(
        context: Context,
        transferId: Int,
        fileName: String,
        savedPath: String
    ) {
        // Dismiss the progress notification
        NotificationManagerCompat.from(context)
            .cancel(PROGRESS_NOTIFICATION_BASE + transferId)
        // Dismiss the request notification
        NotificationManagerCompat.from(context)
            .cancel(REQUEST_NOTIFICATION_BASE + transferId)

        val notification = NotificationCompat.Builder(context, CHANNEL_TRANSFER)
            .setSmallIcon(android.R.drawable.stat_sys_download_done)
            .setContentTitle("Transfer Complete")
            .setContentText("$fileName saved to JustDrop folder")
            .setStyle(NotificationCompat.BigTextStyle()
                .bigText("$fileName saved to:\n$savedPath"))
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setAutoCancel(true)
            .build()

        try {
            NotificationManagerCompat.from(context)
                .notify(COMPLETE_NOTIFICATION_BASE + transferId, notification)
        } catch (_: SecurityException) { }
    }

    /**
     * Show transfer failed notification.
     */
    fun showFailed(
        context: Context,
        transferId: Int,
        reason: String
    ) {
        // Dismiss progress
        NotificationManagerCompat.from(context)
            .cancel(PROGRESS_NOTIFICATION_BASE + transferId)

        val notification = NotificationCompat.Builder(context, CHANNEL_TRANSFER)
            .setSmallIcon(android.R.drawable.stat_notify_error)
            .setContentTitle("Transfer Failed")
            .setContentText(reason)
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setAutoCancel(true)
            .build()

        try {
            NotificationManagerCompat.from(context)
                .notify(COMPLETE_NOTIFICATION_BASE + transferId, notification)
        } catch (_: SecurityException) { }
    }
}

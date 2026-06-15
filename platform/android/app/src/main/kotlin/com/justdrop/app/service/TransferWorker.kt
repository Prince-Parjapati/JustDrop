package com.justdrop.app.service

import android.content.Context
import android.util.Log
import androidx.work.*
import java.util.concurrent.TimeUnit

/**
 * WorkManager worker for resilient background file transfers.
 * Survives process death and handles retry with exponential backoff.
 */
class TransferWorker(
    context: Context,
    params: WorkerParameters,
) : CoroutineWorker(context, params) {
    companion object {
        private const val TAG = "TransferWorker"
        const val KEY_PEER_ID = "peer_id"
        const val KEY_FILE_PATHS = "file_paths"
        const val KEY_TRANSFER_ID = "transfer_id"

        /**
         * Enqueue a resilient file transfer that survives process death.
         */
        fun enqueue(
            context: Context,
            peerId: String,
            filePaths: List<String>,
        ): String {
            val transferId =
                java.util.UUID
                    .randomUUID()
                    .toString()

            val data =
                workDataOf(
                    KEY_PEER_ID to peerId,
                    KEY_FILE_PATHS to filePaths.toTypedArray(),
                    KEY_TRANSFER_ID to transferId,
                )

            val constraints =
                Constraints
                    .Builder()
                    .setRequiredNetworkType(NetworkType.CONNECTED)
                    .build()

            val request =
                OneTimeWorkRequestBuilder<TransferWorker>()
                    .setInputData(data)
                    .setConstraints(constraints)
                    .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 10, TimeUnit.SECONDS)
                    .addTag("justdrop_transfer")
                    .addTag(transferId)
                    .build()

            WorkManager
                .getInstance(context)
                .enqueueUniqueWork(
                    transferId,
                    ExistingWorkPolicy.KEEP,
                    request,
                )

            Log.i(TAG, "Transfer enqueued: $transferId to $peerId (${filePaths.size} files)")
            return transferId
        }

        fun cancelTransfer(
            context: Context,
            transferId: String,
        ) {
            WorkManager.getInstance(context).cancelUniqueWork(transferId)
        }

        fun cancelAll(context: Context) {
            WorkManager.getInstance(context).cancelAllWorkByTag("justdrop_transfer")
        }
    }

    override suspend fun doWork(): Result {
        val peerId = inputData.getString(KEY_PEER_ID) ?: return Result.failure()
        val filePaths = inputData.getStringArray(KEY_FILE_PATHS) ?: return Result.failure()
        val transferId = inputData.getString(KEY_TRANSFER_ID) ?: return Result.failure()

        Log.i(TAG, "Starting transfer $transferId to $peerId")

        return try {
            // Set foreground info for long-running transfers
            setForeground(createForegroundInfo(transferId))

            // Build JSON array of paths
            val pathsJson = org.json.JSONArray(filePaths.toList()).toString()

            // Execute via Rust engine
            val result =
                com.justdrop.app.JustBridge
                    .sendFiles(peerId, pathsJson)

            if (result == 0) {
                Log.i(TAG, "Transfer $transferId completed")
                Result.success()
            } else {
                Log.e(TAG, "Transfer $transferId failed: code $result")
                if (runAttemptCount < 3) Result.retry() else Result.failure()
            }
        } catch (e: Exception) {
            Log.e(TAG, "Transfer $transferId exception", e)
            if (runAttemptCount < 3) Result.retry() else Result.failure()
        }
    }

    private fun createForegroundInfo(transferId: String): ForegroundInfo {
        val notification =
            androidx.core.app.NotificationCompat
                .Builder(
                    applicationContext,
                    "justdrop_transfer",
                ).setContentTitle("Sending files...")
                .setSmallIcon(android.R.drawable.ic_menu_upload)
                .setOngoing(true)
                .setProgress(100, 0, true)
                .build()

        return ForegroundInfo(transferId.hashCode(), notification)
    }
}

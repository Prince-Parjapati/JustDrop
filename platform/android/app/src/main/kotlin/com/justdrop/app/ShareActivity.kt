package com.justdrop.app

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import org.json.JSONArray

/**
 * Transparent activity that receives ACTION_SEND intents from the Share Sheet.
 *
 * Extracts file URIs, resolves them to real paths, shows the device picker,
 * and initiates the transfer via the Rust engine.
 */
class ShareActivity : ComponentActivity() {
    companion object {
        private const val TAG = "ShareActivity"
        private const val REQUEST_DEVICE_PICK = 1001
    }

    private var pendingFiles: List<String> = emptyList()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Ensure the service is running
        val serviceIntent = Intent(this, JustDropService::class.java)
        startForegroundService(serviceIntent)

        // Extract file URIs from the intent
        val uris = extractUris(intent)
        if (uris.isEmpty()) {
            Log.w(TAG, "No files to share")
            finish()
            return
        }

        // Resolve URIs to file paths
        pendingFiles = uris.mapNotNull { resolveUri(it) }
        if (pendingFiles.isEmpty()) {
            Log.e(TAG, "Could not resolve any file URIs")
            finish()
            return
        }

        Log.i(TAG, "Sharing ${pendingFiles.size} files")

        // Check for Android 13+ nearby devices permission
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            if (checkSelfPermission(android.Manifest.permission.NEARBY_WIFI_DEVICES) !=
                android.content.pm.PackageManager.PERMISSION_GRANTED
            ) {
                requestPermissions(arrayOf(android.Manifest.permission.NEARBY_WIFI_DEVICES), 1002)
                return
            }
        }

        showDevicePicker()
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == 1002) {
            // Proceed regardless, discovery might still work if network allows it
            showDevicePicker()
        }
    }

    private fun showDevicePicker() {
        val pickerIntent = Intent(this, DevicePickerActivity::class.java)
        startActivityForResult(pickerIntent, REQUEST_DEVICE_PICK)
    }

    override fun onActivityResult(
        requestCode: Int,
        resultCode: Int,
        data: Intent?,
    ) {
        super.onActivityResult(requestCode, resultCode, data)

        if (requestCode == REQUEST_DEVICE_PICK && resultCode == RESULT_OK) {
            val peerId = data?.getStringExtra("peer_id") ?: return finish()

            // Build JSON array of file paths
            val pathsJson = JSONArray(pendingFiles).toString()

            // Send via Rust engine
            val result = JustBridge.sendFiles(peerId, pathsJson)
            if (result != 0) {
                Log.e(TAG, "Send failed: $result")
            } else {
                Log.i(TAG, "Transfer initiated to peer $peerId")
            }
        }

        finish()
    }

    private fun extractUris(intent: Intent): List<Uri> =
        when (intent.action) {
            Intent.ACTION_SEND -> {
                val uri = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
                listOfNotNull(uri)
            }

            Intent.ACTION_SEND_MULTIPLE -> {
                intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM) ?: emptyList()
            }

            else -> {
                emptyList()
            }
        }

    private fun resolveUri(uri: Uri): String? {
        return try {
            // Copy content URI to a temp file for access by Rust
            val inputStream = contentResolver.openInputStream(uri) ?: return null
            val fileName = uri.lastPathSegment ?: "shared_file"
            val tempFile = java.io.File(cacheDir, fileName)

            tempFile.outputStream().use { output ->
                inputStream.copyTo(output)
            }

            tempFile.absolutePath
        } catch (e: Exception) {
            Log.e(TAG, "Failed to resolve URI: $uri", e)
            null
        }
    }
}

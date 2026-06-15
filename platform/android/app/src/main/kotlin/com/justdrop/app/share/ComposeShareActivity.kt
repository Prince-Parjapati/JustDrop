package com.justdrop.app.share

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.*
import com.justdrop.app.JustBridge
import com.justdrop.app.service.JustDropForegroundService
import com.justdrop.app.ui.screens.DevicePickerSheet
import com.justdrop.app.ui.screens.DeviceUiModel
import com.justdrop.app.ui.theme.JustDropTheme
import org.json.JSONArray

/**
 * Compose-based share activity that receives ACTION_SEND intents.
 * Shows a bottom-sheet device picker and initiates transfer.
 */
class ComposeShareActivity : ComponentActivity() {

    companion object {
        private const val TAG = "ComposeShareActivity"
    }

    private var pendingFiles: List<String> = emptyList()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // Start service if not running
        startForegroundService(Intent(this, JustDropForegroundService::class.java))

        // Extract files
        val uris = extractUris(intent)
        if (uris.isEmpty()) {
            Log.w(TAG, "No files to share")
            finish()
            return
        }

        pendingFiles = uris.mapNotNull { resolveUri(it) }
        if (pendingFiles.isEmpty()) {
            Log.e(TAG, "Could not resolve any file URIs")
            finish()
            return
        }

        Log.i(TAG, "Sharing ${pendingFiles.size} files")

        setContent {
            JustDropTheme {
                var devices by remember { mutableStateOf<List<DeviceUiModel>>(emptyList()) }

                LaunchedEffect(Unit) {
                    while (true) {
                        devices = parsePeers(JustBridge.getPeers())
                        kotlinx.coroutines.delay(1000)
                    }
                }

                DevicePickerSheet(
                    devices = devices,
                    isScanning = true,
                    onDeviceSelected = { deviceId ->
                        sendFiles(deviceId)
                        finish()
                    },
                    onCancel = {
                        setResult(RESULT_CANCELED)
                        finish()
                    },
                )
            }
        }
    }

    private fun sendFiles(peerId: String) {
        val pathsJson = JSONArray(pendingFiles).toString()
        val result = JustBridge.sendFiles(peerId, pathsJson)
        if (result != 0) {
            Log.e(TAG, "Send failed: $result")
        } else {
            Log.i(TAG, "Transfer initiated to $peerId")
        }
    }

    private fun extractUris(intent: Intent): List<Uri> {
        return when (intent.action) {
            Intent.ACTION_SEND -> {
                val uri = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
                listOfNotNull(uri)
            }
            Intent.ACTION_SEND_MULTIPLE -> {
                intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM) ?: emptyList()
            }
            else -> emptyList()
        }
    }

    private fun resolveUri(uri: Uri): String? {
        return try {
            val inputStream = contentResolver.openInputStream(uri) ?: return null
            val fileName = uri.lastPathSegment ?: "shared_file"
            val tempFile = java.io.File(cacheDir, fileName)
            tempFile.outputStream().use { output -> inputStream.copyTo(output) }
            tempFile.absolutePath
        } catch (e: Exception) {
            Log.e(TAG, "Failed to resolve URI: $uri", e)
            null
        }
    }

    private fun parsePeers(json: String?): List<DeviceUiModel> {
        if (json.isNullOrEmpty() || json == "[]") return emptyList()
        return try {
            val arr = JSONArray(json)
            (0 until arr.length()).map { i ->
                val obj = arr.getJSONObject(i)
                DeviceUiModel(
                    deviceId = obj.getString("id"),
                    name = obj.getString("name"),
                    platform = obj.optString("platform", "Unknown"),
                    address = obj.optString("addr", null),
                    presence = "Available",
                    trust = "Unknown",
                    rssi = null,
                )
            }
        } catch (e: Exception) {
            emptyList()
        }
    }
}

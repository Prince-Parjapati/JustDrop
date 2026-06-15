package com.justdrop.app.ui

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.*
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.justdrop.app.JustBridge
import com.justdrop.app.JustDropService
import com.justdrop.app.ui.screens.*
import com.justdrop.app.ui.theme.JustDropTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        setContent {
            JustDropTheme {
                val navController = rememberNavController()
                var isRunning by remember { mutableStateOf(com.justdrop.app.JustDropApp.isServiceRunning) }
                var devices by remember { mutableStateOf<List<DeviceUiModel>>(emptyList()) }
                var transfers by remember { mutableStateOf<List<TransferUiModel>>(emptyList()) }

                // Poll peers
                LaunchedEffect(isRunning) {
                    if (isRunning) {
                        while (true) {
                            val json = JustBridge.getPeers()
                            devices = parsePeers(json)
                            kotlinx.coroutines.delay(1000)
                        }
                    } else {
                        devices = emptyList()
                    }
                }

                NavHost(navController = navController, startDestination = "home") {
                    composable("home") {
                        HomeScreen(
                            isServiceRunning = isRunning,
                            devices = devices,
                            transfers = transfers,
                            onToggleService = {
                                if (isRunning) {
                                    stopService(Intent(this@MainActivity, JustDropService::class.java))
                                    isRunning = false
                                } else {
                                    startForegroundService(Intent(this@MainActivity, JustDropService::class.java))
                                    isRunning = true
                                }
                            },
                            onDeviceClick = { deviceId ->
                                // TODO: trigger send flow
                            },
                            onCancelTransfer = { transferId ->
                                JustBridge.rejectTransfer(transferId)
                            },
                            onSettingsClick = {
                                navController.navigate("settings")
                            },
                        )
                    }
                    composable("settings") {
                        SettingsScreen(
                            deviceName = android.os.Build.MODEL,
                            fingerprint = "TODO",
                            onBack = { navController.popBackStack() },
                        )
                    }
                }
            }
        }
    }

    private fun parsePeers(json: String?): List<DeviceUiModel> {
        if (json.isNullOrEmpty() || json == "[]") return emptyList()
        return try {
            val arr = org.json.JSONArray(json)
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

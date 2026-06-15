package com.justdrop.app

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.util.Log
import android.view.Gravity
import android.view.ViewGroup
import android.widget.*
import org.json.JSONArray

/**
 * Bottom-sheet-style activity showing discovered peer devices.
 *
 * Returns the selected peer ID via the result intent.
 */
class DevicePickerActivity : Activity() {
    companion object {
        private const val TAG = "DevicePicker"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Simple list layout
        val layout =
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                setPadding(32, 32, 32, 32)
                setBackgroundColor(0xFFFFFFFF.toInt())
            }

        val title =
            TextView(this).apply {
                text = "Send to..."
                textSize = 20f
                setPadding(0, 0, 0, 24)
                gravity = Gravity.CENTER
            }
        layout.addView(title)

        // Dynamic layout for peers
        val peersContainer =
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
            }
        layout.addView(peersContainer)

        // Poll for peers periodically
        val handler = android.os.Handler(android.os.Looper.getMainLooper())
        var currentPeersJson = ""

        val runnable =
            object : Runnable {
                override fun run() {
                    val peersJson = JustBridge.getPeers()
                    if (peersJson != currentPeersJson) {
                        currentPeersJson = peersJson ?: ""
                        updatePeersUI(peersContainer, currentPeersJson)
                    }
                    handler.postDelayed(this, 1000)
                }
            }
        handler.post(runnable)

        // Cancel button
        val cancel =
            Button(this).apply {
                text = "Cancel"
                layoutParams =
                    LinearLayout
                        .LayoutParams(
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            ViewGroup.LayoutParams.WRAP_CONTENT,
                        ).apply { setMargins(0, 24, 0, 0) }
                setOnClickListener {
                    handler.removeCallbacks(runnable)
                    setResult(RESULT_CANCELED)
                    finish()
                }
            }
        layout.addView(cancel)

        val scroll = ScrollView(this)
        scroll.addView(layout)
        setContentView(scroll)
    }

    private fun updatePeersUI(
        container: LinearLayout,
        peersJson: String,
    ) {
        container.removeAllViews()
        if (peersJson.isEmpty() || peersJson == "[]") {
            val empty =
                TextView(this).apply {
                    text = "Looking for nearby devices...\nMake sure both devices have JustDrop turned on."
                    textSize = 16f
                    gravity = Gravity.CENTER
                    setPadding(0, 48, 0, 48)
                }
            container.addView(empty)
        } else {
            try {
                val peers = JSONArray(peersJson)
                for (i in 0 until peers.length()) {
                    val peer = peers.getJSONObject(i)
                    val peerId = peer.getString("id")
                    val peerName = peer.getString("name")
                    val platform = peer.optString("platform", "Unknown")
                    val addr = peer.getString("addr")

                    val button =
                        Button(this).apply {
                            text = "$peerName\n$platform • $addr"
                            textSize = 14f
                            isAllCaps = false
                            setPadding(16, 16, 16, 16)
                            layoutParams =
                                LinearLayout
                                    .LayoutParams(
                                        ViewGroup.LayoutParams.MATCH_PARENT,
                                        ViewGroup.LayoutParams.WRAP_CONTENT,
                                    ).apply {
                                        setMargins(0, 8, 0, 8)
                                    }

                            setOnClickListener {
                                val result =
                                    Intent().apply {
                                        putExtra("peer_id", peerId)
                                    }
                                setResult(RESULT_OK, result)
                                finish()
                            }
                        }
                    container.addView(button)
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to parse peers JSON", e)
            }
        }
    }
}

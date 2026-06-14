package com.rustdrop.app

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
        val layout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(32, 32, 32, 32)
            setBackgroundColor(0xFFFFFFFF.toInt())
        }

        val title = TextView(this).apply {
            text = "Send to..."
            textSize = 20f
            setPadding(0, 0, 0, 24)
            gravity = Gravity.CENTER
        }
        layout.addView(title)

        // Get peers from Rust engine
        val peersJson = RustBridge.getPeers()
        if (peersJson.isNullOrEmpty()) {
            val empty = TextView(this).apply {
                text = "No devices found.\nMake sure both devices are on the same network."
                textSize = 16f
                gravity = Gravity.CENTER
                setPadding(0, 48, 0, 48)
            }
            layout.addView(empty)
        } else {
            try {
                val peers = JSONArray(peersJson)
                for (i in 0 until peers.length()) {
                    val peer = peers.getJSONObject(i)
                    val peerId = peer.getString("id")
                    val peerName = peer.getString("name")
                    val platform = peer.optString("platform", "Unknown")
                    val addr = peer.getString("addr")

                    val button = Button(this).apply {
                        text = "$peerName\n$platform • $addr"
                        textSize = 14f
                        isAllCaps = false
                        setPadding(16, 16, 16, 16)
                        layoutParams = LinearLayout.LayoutParams(
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            ViewGroup.LayoutParams.WRAP_CONTENT
                        ).apply {
                            setMargins(0, 8, 0, 8)
                        }

                        setOnClickListener {
                            val result = Intent().apply {
                                putExtra("peer_id", peerId)
                            }
                            setResult(RESULT_OK, result)
                            finish()
                        }
                    }
                    layout.addView(button)
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to parse peers JSON", e)
            }
        }

        // Cancel button
        val cancel = Button(this).apply {
            text = "Cancel"
            setOnClickListener {
                setResult(RESULT_CANCELED)
                finish()
            }
        }
        layout.addView(cancel)

        val scroll = ScrollView(this)
        scroll.addView(layout)
        setContentView(scroll)
    }
}

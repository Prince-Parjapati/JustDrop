package com.justdrop.app.hotspot

import android.content.Context
import android.net.wifi.WifiManager
import android.os.Handler
import android.os.Looper
import android.util.Log

/**
 * Manages Android LocalOnlyHotspot for offline peer-to-peer connectivity.
 */
class HotspotManager(private val context: Context) {

    companion object {
        private const val TAG = "HotspotManager"
    }

    interface Listener {
        fun onHotspotStarted(ssid: String, passphrase: String)
        fun onHotspotFailed(reason: String)
        fun onHotspotStopped()
    }

    private var reservation: WifiManager.LocalOnlyHotspotReservation? = null
    private var listener: Listener? = null

    fun startHotspot(listener: Listener) {
        this.listener = listener
        val wifiManager = context.applicationContext
            .getSystemService(Context.WIFI_SERVICE) as WifiManager

        try {
            wifiManager.startLocalOnlyHotspot(object : WifiManager.LocalOnlyHotspotCallback() {
                override fun onStarted(res: WifiManager.LocalOnlyHotspotReservation?) {
                    reservation = res
                    val config = res?.wifiConfiguration
                    val ssid = config?.SSID ?: return
                    val pass = config.preSharedKey ?: return
                    Log.i(TAG, "Hotspot started: $ssid")
                    listener.onHotspotStarted(ssid, pass)
                }

                override fun onStopped() {
                    Log.i(TAG, "Hotspot stopped")
                    listener.onHotspotStopped()
                }

                override fun onFailed(reason: Int) {
                    Log.e(TAG, "Hotspot failed: $reason")
                    listener.onHotspotFailed("Error code: $reason")
                }
            }, Handler(Looper.getMainLooper()))
        } catch (e: SecurityException) {
            Log.e(TAG, "Hotspot permission denied", e)
            listener.onHotspotFailed("Permission denied")
        }
    }

    fun stopHotspot() {
        reservation?.close()
        reservation = null
    }
}

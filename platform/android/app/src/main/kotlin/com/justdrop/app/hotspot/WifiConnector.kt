package com.justdrop.app.hotspot

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.wifi.WifiConfiguration
import android.net.wifi.WifiManager
import android.net.wifi.WifiNetworkSpecifier
import android.os.Build
import android.util.Log

/**
 * Connects to a peer's LocalOnlyHotspot using credentials
 * received during the BLE handshake.
 */
class WifiConnector(private val context: Context) {

    companion object {
        private const val TAG = "WifiConnector"
    }

    interface Listener {
        fun onConnected(network: Network)
        fun onFailed(reason: String)
        fun onLost()
    }

    private var networkCallback: ConnectivityManager.NetworkCallback? = null

    fun connect(ssid: String, passphrase: String, listener: Listener) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            connectApi29Plus(ssid, passphrase, listener)
        } else {
            connectLegacy(ssid, passphrase, listener)
        }
    }

    private fun connectApi29Plus(ssid: String, passphrase: String, listener: Listener) {
        val specifier = WifiNetworkSpecifier.Builder()
            .setSsid(ssid)
            .setWpa2Passphrase(passphrase)
            .build()

        val request = NetworkRequest.Builder()
            .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
            .setNetworkSpecifier(specifier)
            .build()

        val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                Log.i(TAG, "Connected to peer hotspot: $ssid")
                // Bind process to this network for QUIC traffic
                cm.bindProcessToNetwork(network)
                listener.onConnected(network)
            }

            override fun onUnavailable() {
                Log.e(TAG, "Peer hotspot unavailable: $ssid")
                listener.onFailed("Network unavailable")
            }

            override fun onLost(network: Network) {
                Log.w(TAG, "Lost connection to peer hotspot: $ssid")
                cm.bindProcessToNetwork(null)
                listener.onLost()
            }
        }

        networkCallback = callback
        cm.requestNetwork(request, callback)
    }

    @Suppress("DEPRECATION")
    private fun connectLegacy(ssid: String, passphrase: String, listener: Listener) {
        val wifiManager = context.applicationContext
            .getSystemService(Context.WIFI_SERVICE) as WifiManager

        val config = WifiConfiguration().apply {
            SSID = "\"$ssid\""
            preSharedKey = "\"$passphrase\""
        }

        val netId = wifiManager.addNetwork(config)
        if (netId == -1) {
            listener.onFailed("Failed to add network config")
            return
        }

        wifiManager.disconnect()
        val success = wifiManager.enableNetwork(netId, true)
        if (success) {
            wifiManager.reconnect()
            Log.i(TAG, "Connecting to $ssid (legacy)")
        } else {
            listener.onFailed("enableNetwork failed")
        }
    }

    fun disconnect() {
        val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        cm.bindProcessToNetwork(null)
        networkCallback?.let { cm.unregisterNetworkCallback(it) }
        networkCallback = null
    }
}

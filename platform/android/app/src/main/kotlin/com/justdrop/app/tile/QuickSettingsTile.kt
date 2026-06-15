package com.justdrop.app.tile

import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.util.Log
import com.justdrop.app.JustDropApp
import com.justdrop.app.service.JustDropForegroundService

/**
 * Quick Settings Tile for toggling JustDrop service from the notification shade.
 */
class QuickSettingsTile : TileService() {

    companion object {
        private const val TAG = "JustDropTile"
    }

    override fun onStartListening() {
        super.onStartListening()
        updateTile()
    }

    override fun onClick() {
        super.onClick()

        if (JustDropApp.isServiceRunning) {
            // Stop the service
            val stopIntent = Intent(this, JustDropForegroundService::class.java)
            stopService(stopIntent)
            Log.i(TAG, "Service stopped via QS tile")
        } else {
            // Start the service
            val startIntent = Intent(this, JustDropForegroundService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                startForegroundService(startIntent)
            } else {
                startService(startIntent)
            }
            Log.i(TAG, "Service started via QS tile")
        }

        updateTile()
    }

    private fun updateTile() {
        qsTile?.let { tile ->
            if (JustDropApp.isServiceRunning) {
                tile.state = Tile.STATE_ACTIVE
                tile.label = "JustDrop"
                tile.subtitle = "Ready"
            } else {
                tile.state = Tile.STATE_INACTIVE
                tile.label = "JustDrop"
                tile.subtitle = "Off"
            }
            tile.updateTile()
        }
    }
}

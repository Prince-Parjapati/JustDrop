package com.rustdrop.app

import android.content.ComponentName
import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.util.Log

/**
 * Quick Settings tile for toggling RustDrop on/off from the Android
 * notification shade, similar to AirDrop in the iOS Control Center.
 *
 * Pull down the notification shade, tap "Edit" on the quick settings
 * panel, and drag the RustDrop tile into place.
 */
class RustDropTileService : TileService() {

    companion object {
        private const val TAG = "RustDropTile"
    }

    override fun onStartListening() {
        super.onStartListening()
        updateTileState()
    }

    override fun onClick() {
        super.onClick()
        val running = RustDropApp.isServiceRunning
        if (running) {
            stopRustDrop()
        } else {
            startRustDrop()
        }
        updateTileState()
    }

    private fun startRustDrop() {
        Log.i(TAG, "Starting RustDrop from quick settings")
        val intent = Intent(applicationContext, RustDropService::class.java)
        applicationContext.startForegroundService(intent)
        RustDropApp.isServiceRunning = true
    }

    private fun stopRustDrop() {
        Log.i(TAG, "Stopping RustDrop from quick settings")
        val intent = Intent(applicationContext, RustDropService::class.java)
        applicationContext.stopService(intent)
        RustDropApp.isServiceRunning = false
    }

    private fun updateTileState() {
        val tile = qsTile ?: return
        val active = RustDropApp.isServiceRunning

        tile.state = if (active) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
        tile.label = "RustDrop"
        tile.subtitle = if (active) "Receiving" else "Off"

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            tile.stateDescription = if (active) "RustDrop is active and receiving files" else "RustDrop is off"
        }

        tile.updateTile()
    }
}

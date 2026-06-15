package com.justdrop.app

import android.content.ComponentName
import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.util.Log

/**
 * Quick Settings tile for toggling JustDrop on/off from the Android
 * notification shade, similar to AirDrop in the iOS Control Center.
 *
 * Pull down the notification shade, tap "Edit" on the quick settings
 * panel, and drag the JustDrop tile into place.
 */
class JustDropTileService : TileService() {
    companion object {
        private const val TAG = "JustDropTile"
    }

    override fun onStartListening() {
        super.onStartListening()
        updateTileState()
    }

    override fun onClick() {
        super.onClick()
        val running = JustDropApp.isServiceRunning
        if (running) {
            stopJustDrop()
        } else {
            startJustDrop()
        }
        updateTileState()
    }

    private fun startJustDrop() {
        Log.i(TAG, "Starting JustDrop from quick settings")
        val intent = Intent(applicationContext, JustDropService::class.java)
        applicationContext.startForegroundService(intent)
        JustDropApp.isServiceRunning = true
    }

    private fun stopJustDrop() {
        Log.i(TAG, "Stopping JustDrop from quick settings")
        val intent = Intent(applicationContext, JustDropService::class.java)
        applicationContext.stopService(intent)
        JustDropApp.isServiceRunning = false
    }

    private fun updateTileState() {
        val tile = qsTile ?: return
        val active = JustDropApp.isServiceRunning

        tile.state = if (active) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
        tile.label = "JustDrop"
        tile.subtitle = if (active) "Receiving" else "Off"

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            tile.stateDescription = if (active) "JustDrop is active and receiving files" else "JustDrop is off"
        }

        tile.updateTile()
    }
}

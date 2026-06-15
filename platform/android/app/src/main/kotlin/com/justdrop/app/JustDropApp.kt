package com.justdrop.app

import android.app.Application

/**
 * Application subclass that tracks global JustDrop state and
 * creates notification channels on startup.
 */
class JustDropApp : Application() {

    companion object {
        /**
         * Whether the JustDrop foreground service is currently running.
         * Updated by [JustDropService], [JustDropTileService], and [BootReceiver].
         */
        @Volatile
        @JvmStatic
        var isServiceRunning: Boolean = false
    }

    override fun onCreate() {
        super.onCreate()
        TransferNotifications.createChannels(this)
    }
}

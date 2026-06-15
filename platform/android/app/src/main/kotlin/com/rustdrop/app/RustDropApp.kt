package com.rustdrop.app

import android.app.Application

/**
 * Application subclass that tracks global RustDrop state.
 *
 * The Quick Settings tile and BootReceiver both check
 * [isServiceRunning] to decide the current toggle state.
 */
class RustDropApp : Application() {

    companion object {
        /**
         * Whether the RustDrop foreground service is currently running.
         * Updated by [RustDropService], [RustDropTileService], and [BootReceiver].
         */
        @Volatile
        @JvmStatic
        var isServiceRunning: Boolean = false
    }

    override fun onCreate() {
        super.onCreate()
    }
}

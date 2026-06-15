package com.justdrop.app

/**
 * JNI bridge to the Rust JustDrop FFI library.
 *
 * All native methods map directly to the C ABI functions exported by
 * `justdrop-ffi`. The library is loaded once when this object is first accessed.
 */
object JustBridge {
    init {
        System.loadLibrary("justdrop_ffi")
    }

    // ── Lifecycle ──

    /** Initialize the Rust engine. Returns 0 on success. */
    external fun init(configPath: String?): Int

    /** Shut down the Rust engine. */
    external fun shutdown(): Int

    // ── Discovery ──

    /** Start mDNS discovery. Returns 0 on success. */
    external fun startDiscovery(): Int

    /** Get discovered peers as a JSON array string. Caller must NOT free. */
    external fun getPeers(): String?

    // ── Transfer ──

    /**
     * Send files to a peer.
     * @param peerId peer ID from discovery
     * @param filePathsJson JSON array of file path strings
     * @return 0 on success (transfer started in background)
     */
    external fun sendFiles(
        peerId: String,
        filePathsJson: String,
    ): Int

    /** Accept an incoming transfer request. */
    external fun acceptTransfer(transferId: String): Int

    /** Reject an incoming transfer request. */
    external fun rejectTransfer(transferId: String): Int

    // ── Android-specific ──

    /** Set the Android downloads directory for received files. */
    external fun setDownloadsDir(path: String): Int

    /** Set the Android data directory for key storage. */
    external fun setDataDir(path: String): Int
}

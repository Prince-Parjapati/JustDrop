package com.justdrop.app.preview

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.media.MediaMetadataRetriever
import android.media.ThumbnailUtils
import android.os.Build
import android.provider.MediaStore
import android.util.Log
import android.util.Size

/**
 * Generates thumbnails for file transfer previews.
 * Supports images, videos, and falls back to mime-type icons.
 */
object PreviewGenerator {

    private const val TAG = "PreviewGenerator"
    private const val THUMB_SIZE = 256

    /**
     * Generate a thumbnail bitmap for the given file path.
     * Returns null if thumbnail generation fails.
     */
    fun generateThumbnail(context: Context, filePath: String, mimeType: String): Bitmap? {
        return try {
            when {
                mimeType.startsWith("image/") -> generateImageThumbnail(filePath)
                mimeType.startsWith("video/") -> generateVideoThumbnail(filePath)
                else -> null
            }
        } catch (e: Exception) {
            Log.w(TAG, "Thumbnail generation failed for $filePath", e)
            null
        }
    }

    private fun generateImageThumbnail(filePath: String): Bitmap? {
        val options = BitmapFactory.Options().apply {
            inJustDecodeBounds = true
        }
        BitmapFactory.decodeFile(filePath, options)

        // Calculate sample size
        val scaleFactor = maxOf(
            options.outWidth / THUMB_SIZE,
            options.outHeight / THUMB_SIZE,
            1
        )

        val decodeOptions = BitmapFactory.Options().apply {
            inSampleSize = scaleFactor
        }

        val bitmap = BitmapFactory.decodeFile(filePath, decodeOptions) ?: return null
        return ThumbnailUtils.extractThumbnail(bitmap, THUMB_SIZE, THUMB_SIZE)
    }

    private fun generateVideoThumbnail(filePath: String): Bitmap? {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            return ThumbnailUtils.createVideoThumbnail(
                java.io.File(filePath),
                Size(THUMB_SIZE, THUMB_SIZE),
                null,
            )
        }

        @Suppress("DEPRECATION")
        return ThumbnailUtils.createVideoThumbnail(
            filePath,
            MediaStore.Images.Thumbnails.MINI_KIND,
        )
    }

    /**
     * Format file size for display.
     */
    fun formatFileSize(bytes: Long): String {
        return when {
            bytes < 1024 -> "$bytes B"
            bytes < 1024 * 1024 -> "${bytes / 1024} KB"
            bytes < 1024 * 1024 * 1024 -> String.format("%.1f MB", bytes / (1024.0 * 1024))
            else -> String.format("%.2f GB", bytes / (1024.0 * 1024 * 1024))
        }
    }

    /**
     * Get a display-friendly MIME type description.
     */
    fun mimeTypeLabel(mimeType: String): String {
        return when {
            mimeType.startsWith("image/") -> "Image"
            mimeType.startsWith("video/") -> "Video"
            mimeType.startsWith("audio/") -> "Audio"
            mimeType == "application/pdf" -> "PDF"
            mimeType.contains("zip") || mimeType.contains("compressed") -> "Archive"
            mimeType.startsWith("text/") -> "Text"
            else -> "File"
        }
    }
}

import QuickLookThumbnailing
import AppKit
import os.log

/// Generates thumbnails for file previews using QuickLookThumbnailing framework.
class ThumbnailGenerator {
    private static let logger = Logger(subsystem: "com.justdrop", category: "Thumbnail")

    /// Generate a thumbnail for a file at the given path.
    static func generate(for url: URL, size: CGSize = CGSize(width: 256, height: 256),
                         completion: @escaping (NSImage?) -> Void) {
        let request = QLThumbnailGenerator.Request(
            fileAt: url,
            size: size,
            scale: NSScreen.main?.backingScaleFactor ?? 2.0,
            representationTypes: .thumbnail
        )

        QLThumbnailGenerator.shared.generateRepresentations(for: request) { thumbnail, _, error in
            if let error = error {
                logger.warning("Thumbnail failed for \(url.lastPathComponent): \(error.localizedDescription)")
                DispatchQueue.main.async { completion(nil) }
                return
            }

            guard let cgImage = thumbnail?.cgImage else {
                DispatchQueue.main.async { completion(nil) }
                return
            }

            let nsImage = NSImage(cgImage: cgImage, size: size)
            DispatchQueue.main.async { completion(nsImage) }
        }
    }

    /// Synchronous thumbnail generation (blocking).
    static func generateSync(for url: URL, size: CGSize = CGSize(width: 256, height: 256)) -> NSImage? {
        let semaphore = DispatchSemaphore(value: 0)
        var result: NSImage?

        generate(for: url, size: size) { image in
            result = image
            semaphore.signal()
        }

        _ = semaphore.wait(timeout: .now() + 5.0)
        return result
    }

    /// Format bytes for display.
    static func formatSize(_ bytes: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(bytes), countStyle: .file)
    }

    /// Get file icon from NSWorkspace.
    static func fileIcon(for url: URL) -> NSImage {
        NSWorkspace.shared.icon(forFile: url.path)
    }
}

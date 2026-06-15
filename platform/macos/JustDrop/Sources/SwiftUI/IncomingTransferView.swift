import SwiftUI

/// View shown when an incoming transfer request is received.
struct IncomingTransferView: View {
    let senderName: String
    let files: [FileItem]
    let totalSize: String
    let onAccept: () -> Void
    let onReject: () -> Void

    @State private var showDetails = false

    var body: some View {
        VStack(spacing: 16) {
            // Icon
            Image(systemName: "arrow.down.circle.fill")
                .font(.system(size: 48))
                .foregroundStyle(.blue)

            Text("Incoming Transfer")
                .font(.title2)
                .fontWeight(.bold)

            Text("from \(senderName)")
                .font(.body)
                .foregroundColor(.secondary)

            // File list
            GroupBox {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(files.prefix(5)) { file in
                        HStack(spacing: 8) {
                            Image(systemName: file.icon)
                                .foregroundColor(.secondary)
                                .frame(width: 16)
                            Text(file.name)
                                .lineLimit(1)
                                .truncationMode(.middle)
                            Spacer()
                            Text(file.sizeFormatted)
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                        .font(.caption)
                    }
                    if files.count > 5 {
                        Text("+\(files.count - 5) more files")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                }
            }

            Text(totalSize)
                .font(.callout)
                .foregroundColor(.secondary)

            // Action buttons
            HStack(spacing: 12) {
                Button(action: onReject) {
                    Label("Decline", systemImage: "xmark")
                        .frame(maxWidth: .infinity)
                }
                .controlSize(.large)
                .keyboardShortcut(.escape)

                Button(action: onAccept) {
                    Label("Accept", systemImage: "checkmark")
                        .frame(maxWidth: .infinity)
                }
                .controlSize(.large)
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.return)
            }
        }
        .padding(24)
        .frame(width: 340)
    }
}

struct FileItem: Identifiable {
    let id = UUID()
    let name: String
    let size: UInt64
    let mimeType: String

    var sizeFormatted: String {
        ByteCountFormatter.string(fromByteCount: Int64(size), countStyle: .file)
    }

    var icon: String {
        if mimeType.hasPrefix("image/") { return "photo" }
        if mimeType.hasPrefix("video/") { return "film" }
        if mimeType.hasPrefix("audio/") { return "music.note" }
        if mimeType == "application/pdf" { return "doc.richtext" }
        if mimeType.contains("zip") || mimeType.contains("compressed") { return "doc.zipper" }
        return "doc"
    }
}

import Cocoa
import SwiftUI

// MARK: - FFI Declarations (same as main app, needed because extension runs in separate process)

@_silgen_name("justdrop_init")
func justdrop_init(_ dataDir: UnsafePointer<CChar>?) -> Int32

@_silgen_name("justdrop_shutdown")
func justdrop_shutdown() -> Int32

@_silgen_name("justdrop_start_discovery")
func justdrop_start_discovery() -> Int32

@_silgen_name("justdrop_get_peers")
func justdrop_get_peers() -> UnsafeMutablePointer<CChar>?

@_silgen_name("justdrop_send_files")
func justdrop_send_files(_ peerId: UnsafePointer<CChar>?, _ pathsJson: UnsafePointer<CChar>?) -> Int32

@_silgen_name("justdrop_free_string")
func justdrop_free_string(_ ptr: UnsafeMutablePointer<CChar>?)

/// Share Extension for macOS — receives files from Finder's Share menu.
class ShareViewController: NSViewController {

    override var nibName: NSNib.Name? { nil }

    private var engineStarted = false

    override func loadView() {
        // Start engine in the extension process
        if !engineStarted {
            let dataDir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
                .first?.appendingPathComponent("com.justdrop").path ?? ""
            dataDir.withCString { justdrop_init($0) }
            justdrop_start_discovery()
            engineStarted = true
        }

        let hostView = NSHostingView(rootView: ShareExtensionView(
            onSend: { [weak self] peerId in
                self?.sendFiles(to: peerId)
            },
            onCancel: { [weak self] in
                self?.cancel()
            }
        ))
        hostView.frame = NSRect(x: 0, y: 0, width: 360, height: 400)
        self.view = hostView
    }

    override func viewDidDisappear() {
        super.viewDidDisappear()
        if engineStarted {
            justdrop_shutdown()
            engineStarted = false
        }
    }

    private func sendFiles(to peerId: String) {
        guard let items = extensionContext?.inputItems as? [NSExtensionItem] else {
            cancel()
            return
        }

        var filePaths: [String] = []

        let group = DispatchGroup()
        for item in items {
            for attachment in item.attachments ?? [] {
                if attachment.hasItemConformingToTypeIdentifier("public.file-url") {
                    group.enter()
                    attachment.loadItem(forTypeIdentifier: "public.file-url", options: nil) { url, _ in
                        if let fileURL = url as? URL {
                            filePaths.append(fileURL.path)
                        }
                        group.leave()
                    }
                }
            }
        }

        group.notify(queue: .main) { [weak self] in
            if !filePaths.isEmpty {
                if let jsonData = try? JSONSerialization.data(withJSONObject: filePaths),
                   let jsonStr = String(data: jsonData, encoding: .utf8) {
                    peerId.withCString { peerPtr in
                        jsonStr.withCString { pathsPtr in
                            justdrop_send_files(peerPtr, pathsPtr)
                        }
                    }
                }
            }
            self?.extensionContext?.completeRequest(returningItems: nil)
        }
    }

    private func cancel() {
        extensionContext?.cancelRequest(withError: NSError(domain: "com.justdrop.share", code: 0))
    }
}

/// SwiftUI view for the share extension sheet.
struct ShareExtensionView: View {
    let onSend: (String) -> Void
    let onCancel: () -> Void

    @State private var peers: [SharePeer] = []
    @State private var isScanning = true
    private let timer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(spacing: 16) {
            HStack {
                Image(systemName: "arrow.up.arrow.down.circle.fill")
                    .font(.title2)
                    .foregroundColor(.accentColor)
                Text("Send with JustDrop")
                    .font(.headline)
            }

            if peers.isEmpty {
                VStack(spacing: 8) {
                    ProgressView()
                    Text("Looking for devices...")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                List(peers) { peer in
                    Button {
                        onSend(peer.id)
                    } label: {
                        HStack {
                            Image(systemName: peer.icon)
                                .font(.title3)
                                .frame(width: 32, height: 32)
                                .background(.quaternary)
                                .clipShape(Circle())
                            VStack(alignment: .leading) {
                                Text(peer.name).fontWeight(.medium)
                                Text(peer.platform)
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                            }
                            Spacer()
                            Image(systemName: "paperplane.fill")
                                .foregroundColor(.accentColor)
                        }
                    }
                    .buttonStyle(.plain)
                }
            }

            Button("Cancel", action: onCancel)
                .keyboardShortcut(.escape)
        }
        .padding()
        .frame(width: 320, height: 360)
        .onReceive(timer) { _ in
            refreshPeers()
        }
    }

    private func refreshPeers() {
        guard let json = justdrop_get_peers() else { return }
        let str = String(cString: json)
        justdrop_free_string(json)
        guard let data = str.data(using: .utf8),
              let arr = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else { return }

        peers = arr.compactMap { dict in
            guard let id = dict["id"] as? String,
                  let name = dict["name"] as? String else { return nil }
            let platform = dict["platform"] as? String ?? "Unknown"
            return SharePeer(id: id, name: name, platform: platform)
        }
    }
}

struct SharePeer: Identifiable {
    let id: String
    let name: String
    let platform: String

    var icon: String {
        switch platform {
        case "MacOS": return "laptopcomputer"
        case "Android": return "candybarphone"
        default: return "desktopcomputer"
        }
    }
}

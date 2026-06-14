import Cocoa

/// macOS Share Extension for RustDrop.
///
/// Appears in the system Share menu when files are selected in Finder,
/// Safari, or other apps. Shows a device picker and sends files via
/// the Rust engine.
class ShareViewController: NSViewController {

    private var selectedFiles: [String] = []
    private var peers: [[String: Any]] = []
    private var tableView: NSTableView!

    override var nibName: NSNib.Name? { nil }

    override func loadView() {
        self.view = NSView(frame: NSRect(x: 0, y: 0, width: 400, height: 300))
    }

    override func viewDidLoad() {
        super.viewDidLoad()

        // Extract shared items
        extractSharedItems()

        // Initialize Rust engine (if not already running via daemon)
        let bridge = RustBridge.shared
        _ = bridge.initialize()
        _ = bridge.startDiscovery()

        // Wait briefly for discovery, then show peers
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [weak self] in
            self?.loadPeers()
            self?.setupUI()
        }
    }

    private func extractSharedItems() {
        guard let items = extensionContext?.inputItems as? [NSExtensionItem] else { return }

        for item in items {
            guard let attachments = item.attachments else { continue }
            for provider in attachments {
                if provider.hasItemConformingToTypeIdentifier("public.file-url") {
                    provider.loadItem(
                        forTypeIdentifier: "public.file-url",
                        options: nil
                    ) { [weak self] item, error in
                        if let url = item as? URL {
                            self?.selectedFiles.append(url.path)
                        }
                    }
                }
            }
        }
    }

    private func loadPeers() {
        peers = RustBridge.shared.getPeers()
    }

    private func setupUI() {
        let container = NSStackView(frame: view.bounds)
        container.orientation = .vertical
        container.spacing = 12
        container.edgeInsets = NSEdgeInsets(top: 16, left: 16, bottom: 16, right: 16)

        // Title
        let title = NSTextField(labelWithString: "Send with RustDrop")
        title.font = .boldSystemFont(ofSize: 16)
        container.addArrangedSubview(title)

        // File count
        let fileLabel = NSTextField(
            labelWithString: "\(selectedFiles.count) file(s) selected"
        )
        fileLabel.font = .systemFont(ofSize: 12)
        fileLabel.textColor = .secondaryLabelColor
        container.addArrangedSubview(fileLabel)

        if peers.isEmpty {
            let empty = NSTextField(
                labelWithString: "No devices found.\nEnsure both devices are on the same network."
            )
            empty.font = .systemFont(ofSize: 14)
            empty.alignment = .center
            container.addArrangedSubview(empty)
        } else {
            // Peer list
            let scrollView = NSScrollView(frame: NSRect(x: 0, y: 0, width: 360, height: 160))
            tableView = NSTableView()

            let nameColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("name"))
            nameColumn.title = "Device"
            nameColumn.width = 200
            tableView.addTableColumn(nameColumn)

            let platformColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("platform"))
            platformColumn.title = "Platform"
            platformColumn.width = 100
            tableView.addTableColumn(platformColumn)

            tableView.delegate = self
            tableView.dataSource = self
            tableView.doubleAction = #selector(sendToSelected)

            scrollView.documentView = tableView
            scrollView.hasVerticalScroller = true

            let constraint = scrollView.heightAnchor.constraint(equalToConstant: 160)
            constraint.isActive = true

            container.addArrangedSubview(scrollView)
        }

        // Buttons
        let buttonStack = NSStackView()
        buttonStack.orientation = .horizontal
        buttonStack.spacing = 8

        let cancelButton = NSButton(title: "Cancel", target: self, action: #selector(cancel))
        buttonStack.addArrangedSubview(cancelButton)

        if !peers.isEmpty {
            let sendButton = NSButton(title: "Send", target: self, action: #selector(sendToSelected))
            sendButton.bezelStyle = .rounded
            sendButton.keyEquivalent = "\r"
            buttonStack.addArrangedSubview(sendButton)
        }

        container.addArrangedSubview(buttonStack)

        view.addSubview(container)
        container.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            container.topAnchor.constraint(equalTo: view.topAnchor),
            container.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            container.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            container.trailingAnchor.constraint(equalTo: view.trailingAnchor),
        ])
    }

    @objc private func sendToSelected() {
        let row = tableView?.selectedRow ?? -1
        guard row >= 0 && row < peers.count else { return }

        let peer = peers[row]
        guard let peerId = peer["id"] as? String else { return }

        let success = RustBridge.shared.sendFiles(peerId: peerId, filePaths: selectedFiles)
        if success {
            NSLog("RustDrop Share: Transfer initiated to \(peerId)")
        } else {
            NSLog("RustDrop Share: Transfer failed")
        }

        extensionContext?.completeRequest(returningItems: nil)
    }

    @objc private func cancel() {
        extensionContext?.cancelRequest(withError: NSError(
            domain: "com.rustdrop.share",
            code: 0,
            userInfo: [NSLocalizedDescriptionKey: "User cancelled"]
        ))
    }
}

// MARK: - NSTableViewDataSource & Delegate

extension ShareViewController: NSTableViewDataSource, NSTableViewDelegate {
    func numberOfRows(in tableView: NSTableView) -> Int {
        return peers.count
    }

    func tableView(
        _ tableView: NSTableView,
        viewFor tableColumn: NSTableColumn?,
        row: Int
    ) -> NSView? {
        let peer = peers[row]
        let text: String

        switch tableColumn?.identifier.rawValue {
        case "name":
            text = peer["name"] as? String ?? "Unknown"
        case "platform":
            text = peer["platform"] as? String ?? "Unknown"
        default:
            return nil
        }

        let cell = NSTextField(labelWithString: text)
        cell.font = .systemFont(ofSize: 13)
        return cell
    }
}

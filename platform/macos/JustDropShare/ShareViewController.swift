import Cocoa

class ShareViewController: NSViewController {

    private var selectedFiles: [String] = []
    private var peers: [[String: Any]] = []
    private var tableView: NSTableView!
    private var statusLabel: NSTextField!
    private var container: NSStackView!
    private var scrollView: NSScrollView!

    override var nibName: NSNib.Name? { nil }

    override func loadView() {
        self.view = NSView(frame: NSRect(x: 0, y: 0, width: 400, height: 300))
    }

    override func viewDidLoad() {
        super.viewDidLoad()

        extractSharedItems()

        let bridge = JustBridge.shared
        if let configPath = createTempConfig() {
            _ = bridge.initialize(configPath: configPath)
        } else {
            _ = bridge.initialize()
        }
        _ = bridge.startDiscovery()

        setupUI()
        loadPeers()
    }

    private func createTempConfig() -> String? {
        let toml = """
        [network]
        listen_port = 0
        """
        let tempDir = FileManager.default.temporaryDirectory
        let fileURL = tempDir.appendingPathComponent("justdrop_share_config_\(UUID().uuidString).toml")
        do {
            try toml.write(to: fileURL, atomically: true, encoding: .utf8)
            return fileURL.path
        } catch {
            return nil
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

    private var timer: Timer?

    override func viewDidDisappear() {
        super.viewDidDisappear()
        timer?.invalidate()
    }

    private func loadPeers() {
        timer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            guard let self = self else { return }
            let newPeers = JustBridge.shared.getPeers()
            if self.peers.count != newPeers.count || String(describing: self.peers) != String(describing: newPeers) {
                self.peers = newPeers
                self.updateVisibility()
            }
        }
        timer?.fire()
    }

    private func setupUI() {
        container = NSStackView(frame: view.bounds)
        container.orientation = .vertical
        container.spacing = 12
        container.edgeInsets = NSEdgeInsets(top: 16, left: 16, bottom: 16, right: 16)
        container.translatesAutoresizingMaskIntoConstraints = false

        let title = NSTextField(labelWithString: "Send with JustDrop")
        title.font = .boldSystemFont(ofSize: 16)
        container.addArrangedSubview(title)

        statusLabel = NSTextField(labelWithString: "Searching for nearby devices...\nMake sure JustDrop is turned on.")
        statusLabel.alignment = .center
        statusLabel.font = .systemFont(ofSize: 14)
        statusLabel.textColor = .secondaryLabelColor
        container.addArrangedSubview(statusLabel)

        scrollView = NSScrollView(frame: NSRect(x: 0, y: 0, width: 360, height: 160))
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
        scrollView.heightAnchor.constraint(equalToConstant: 160).isActive = true
        container.addArrangedSubview(scrollView)

        let buttonStack = NSStackView()
        buttonStack.orientation = .horizontal
        buttonStack.spacing = 8

        let cancelButton = NSButton(title: "Cancel", target: self, action: #selector(cancel))
        buttonStack.addArrangedSubview(cancelButton)

        let sendButton = NSButton(title: "Send", target: self, action: #selector(sendToSelected))
        sendButton.bezelStyle = .rounded
        sendButton.keyEquivalent = "\r"
        buttonStack.addArrangedSubview(sendButton)

        container.addArrangedSubview(buttonStack)

        view.addSubview(container)
        NSLayoutConstraint.activate([
            container.topAnchor.constraint(equalTo: view.topAnchor),
            container.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            container.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            container.trailingAnchor.constraint(equalTo: view.trailingAnchor),
        ])

        updateVisibility()
    }

    private func updateVisibility() {
        if peers.isEmpty {
            statusLabel.isHidden = false
            scrollView.isHidden = true
        } else {
            statusLabel.isHidden = true
            scrollView.isHidden = false
            tableView.reloadData()
        }
    }

    @objc private func sendToSelected() {
        let row = tableView.selectedRow
        guard row >= 0 && row < peers.count else { return }

        let peer = peers[row]
        guard let peerId = peer["id"] as? String else { return }

        let success = JustBridge.shared.sendFiles(peerId: peerId, filePaths: selectedFiles)
        if success {
            NSLog("JustDrop Share: Transfer initiated to \(peerId)")
        }

        extensionContext?.completeRequest(returningItems: nil)
    }

    @objc private func cancel() {
        extensionContext?.cancelRequest(withError: NSError(
            domain: "com.justdrop.share",
            code: 0,
            userInfo: [NSLocalizedDescriptionKey: "User cancelled"]
        ))
    }
}

extension ShareViewController: NSTableViewDataSource, NSTableViewDelegate {
    func numberOfRows(in tableView: NSTableView) -> Int {
        return peers.count
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
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

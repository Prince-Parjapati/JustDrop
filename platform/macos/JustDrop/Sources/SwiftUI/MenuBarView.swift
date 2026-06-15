import SwiftUI

/// Menu bar window showing nearby devices and quick actions.
struct MenuBarView: View {
    @ObservedObject var engine: EngineViewModel

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("JustDrop")
                        .font(.headline)
                        .fontWeight(.bold)
                    Text(engine.isRunning ? "\(engine.peers.count) devices nearby" : "Stopped")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                Spacer()
                Toggle("", isOn: $engine.isRunning)
                    .toggleStyle(.switch)
                    .labelsHidden()
                    .controlSize(.small)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)

            Divider()

            // Active transfers
            if !engine.transfers.isEmpty {
                VStack(spacing: 8) {
                    ForEach(engine.transfers) { transfer in
                        TransferRow(transfer: transfer) {
                            engine.cancelTransfer(id: transfer.id)
                        }
                    }
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                Divider()
            }

            // Device list
            if engine.isRunning {
                if engine.peers.isEmpty {
                    VStack(spacing: 8) {
                        ProgressView()
                            .controlSize(.small)
                        Text("Scanning for devices...")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 24)
                } else {
                    ScrollView {
                        LazyVStack(spacing: 4) {
                            ForEach(engine.peers) { peer in
                                PeerRow(peer: peer) {
                                    engine.sendFiles(to: peer.id)
                                }
                            }
                        }
                        .padding(.horizontal, 8)
                        .padding(.vertical, 8)
                    }
                    .frame(maxHeight: 300)
                }
            }

            Divider()

            // Footer
            HStack {
                Button {
                    NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
                } label: {
                    Label("Settings", systemImage: "gearshape")
                }
                .buttonStyle(.plain)
                .font(.caption)

                Spacer()

                Button {
                    NSApplication.shared.terminate(nil)
                } label: {
                    Label("Quit", systemImage: "power")
                }
                .buttonStyle(.plain)
                .font(.caption)
                .foregroundColor(.red)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
        }
        .frame(width: 320)
    }
}

// MARK: - Peer Row

struct PeerRow: View {
    let peer: PeerModel
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 12) {
                // Platform icon with presence dot
                ZStack(alignment: .bottomTrailing) {
                    Image(systemName: peer.platformIcon)
                        .font(.title3)
                        .frame(width: 36, height: 36)
                        .background(.quaternary)
                        .clipShape(Circle())

                    Circle()
                        .fill(peer.presenceColor)
                        .frame(width: 10, height: 10)
                        .overlay(
                            Circle()
                                .stroke(.background, lineWidth: 2)
                        )
                }

                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 4) {
                        Text(peer.name)
                            .font(.body)
                            .fontWeight(.medium)
                        if let badge = peer.trustBadge {
                            Text(badge)
                                .font(.caption2)
                        }
                    }
                    Text(peer.subtitle)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }

                Spacer()

                if let rssi = peer.rssi {
                    SignalBars(rssi: rssi)
                }

                Image(systemName: "chevron.right")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(.quaternary.opacity(0.5))
        )
    }
}

// MARK: - Transfer Row

struct TransferRow: View {
    let transfer: TransferModel
    let onCancel: () -> Void

    var body: some View {
        VStack(spacing: 6) {
            HStack {
                Image(systemName: transfer.direction == .send ? "arrow.up" : "arrow.down")
                    .foregroundColor(.accentColor)
                    .font(.caption)
                Text(transfer.peerName)
                    .font(.caption)
                    .fontWeight(.medium)
                Spacer()
                Button(action: onCancel) {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(.secondary)
                }
                .buttonStyle(.plain)
            }

            ProgressView(value: transfer.progress)
                .tint(.accentColor)

            HStack {
                Text("\(Int(transfer.progress * 100))%")
                    .font(.caption2)
                    .fontWeight(.bold)
                    .foregroundColor(.accentColor)
                Spacer()
                Text(transfer.speed)
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }
        }
        .padding(8)
        .background(RoundedRectangle(cornerRadius: 8).fill(.quaternary))
    }
}

// MARK: - Signal Bars

struct SignalBars: View {
    let rssi: Int

    var bars: Int {
        switch rssi {
        case (-50)...: return 4
        case (-60)...: return 3
        case (-70)...: return 2
        case (-80)...: return 1
        default: return 0
        }
    }

    var body: some View {
        HStack(spacing: 1) {
            ForEach(0..<4, id: \.self) { i in
                RoundedRectangle(cornerRadius: 1)
                    .fill(i < bars ? Color.accentColor : Color.secondary.opacity(0.3))
                    .frame(width: 3, height: CGFloat(6 + i * 3))
            }
        }
    }
}

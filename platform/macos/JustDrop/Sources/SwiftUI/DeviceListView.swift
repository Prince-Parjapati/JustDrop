import SwiftUI

/// Standalone device list view for the main window (if used outside menu bar).
struct DeviceListView: View {
    @ObservedObject var engine: EngineViewModel

    var body: some View {
        VStack(spacing: 0) {
            // Search/filter bar
            HStack {
                Image(systemName: "magnifyingglass")
                    .foregroundColor(.secondary)
                Text("\(engine.peers.count) devices nearby")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                Spacer()
                if engine.isRunning {
                    ProgressView()
                        .controlSize(.small)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)

            Divider()

            if engine.peers.isEmpty {
                VStack(spacing: 8) {
                    Label("No Devices", systemImage: "antenna.radiowaves.left.and.right.slash")
                        .font(.title2)
                        .foregroundColor(.secondary)
                    Text("Make sure other devices have JustDrop running on the same network")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                List(engine.peers) { peer in
                    DeviceRow(peer: peer) {
                        engine.sendFiles(to: peer.id)
                    }
                }
                .listStyle(.inset)
            }
        }
    }
}

struct DeviceRow: View {
    let peer: PeerModel
    let onSend: () -> Void

    @State private var isHovering = false

    var body: some View {
        HStack(spacing: 12) {
            ZStack(alignment: .bottomTrailing) {
                Image(systemName: peer.platformIcon)
                    .font(.title2)
                    .frame(width: 40, height: 40)
                    .background(.quaternary)
                    .clipShape(Circle())

                Circle()
                    .fill(peer.presenceColor)
                    .frame(width: 10, height: 10)
                    .overlay(Circle().stroke(.background, lineWidth: 2))
            }

            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 4) {
                    Text(peer.name)
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

            Button(action: onSend) {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.title3)
            }
            .buttonStyle(.plain)
            .opacity(isHovering ? 1 : 0.6)
        }
        .padding(.vertical, 4)
        .onHover { isHovering = $0 }
    }
}

import SwiftUI

/// Transfer progress and history view.
struct TransferView: View {
    @ObservedObject var engine: EngineViewModel

    var body: some View {
        VStack(spacing: 0) {
            if engine.transfers.isEmpty {
                VStack(spacing: 8) {
                    Label("No Active Transfers", systemImage: "arrow.up.arrow.down.circle")
                        .font(.title2)
                        .foregroundColor(.secondary)
                    Text("File transfers will appear here")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                List {
                    Section("Active") {
                        ForEach(engine.transfers) { transfer in
                            TransferRowDetail(transfer: transfer) {
                                engine.cancelTransfer(id: transfer.id)
                            }
                        }
                    }
                }
                .listStyle(.inset)
            }
        }
    }
}

struct TransferRowDetail: View {
    let transfer: TransferModel
    let onCancel: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Image(systemName: transfer.direction == .send ? "arrow.up.circle.fill" : "arrow.down.circle.fill")
                    .foregroundColor(.accentColor)
                    .font(.title3)

                VStack(alignment: .leading, spacing: 2) {
                    Text(transfer.direction == .send ? "Sending to \(transfer.peerName)" : "Receiving from \(transfer.peerName)")
                        .fontWeight(.medium)
                    Text(transfer.speed)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }

                Spacer()

                Button(action: onCancel) {
                    Image(systemName: "xmark.circle")
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
                if let eta = transfer.eta {
                    Text("ETA: \(eta)")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
            }
        }
        .padding(.vertical, 4)
    }
}

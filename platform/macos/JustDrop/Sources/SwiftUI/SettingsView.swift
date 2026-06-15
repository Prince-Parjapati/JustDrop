import SwiftUI

/// Settings window with device identity, trust management, and preferences.
struct SettingsView: View {
    @ObservedObject var engine: EngineViewModel
    @State private var autoAcceptTrusted = true
    @State private var startOnLogin = true
    @State private var showNotifications = true

    var body: some View {
        TabView {
            GeneralTab(
                autoAcceptTrusted: $autoAcceptTrusted,
                startOnLogin: $startOnLogin,
                showNotifications: $showNotifications
            )
            .tabItem {
                Label("General", systemImage: "gearshape")
            }

            IdentityTab(engine: engine)
            .tabItem {
                Label("Identity", systemImage: "person.badge.key")
            }

            TrustedDevicesTab(engine: engine)
            .tabItem {
                Label("Devices", systemImage: "laptopcomputer.and.iphone")
            }
        }
        .frame(width: 480, height: 360)
    }
}

struct GeneralTab: View {
    @Binding var autoAcceptTrusted: Bool
    @Binding var startOnLogin: Bool
    @Binding var showNotifications: Bool

    var body: some View {
        Form {
            Section("Transfers") {
                Toggle("Auto-accept from trusted devices", isOn: $autoAcceptTrusted)
                Toggle("Show transfer notifications", isOn: $showNotifications)
            }
            Section("System") {
                Toggle("Start JustDrop on login", isOn: $startOnLogin)
            }
            Section("About") {
                LabeledContent("Version", value: "2.0")
                LabeledContent("Transport", value: "QUIC (Quinn)")
                LabeledContent("Crypto", value: "Ed25519 + ChaCha20")
                LabeledContent("Hashing", value: "BLAKE3")
            }
        }
        .formStyle(.grouped)
        .padding()
    }
}

struct IdentityTab: View {
    @ObservedObject var engine: EngineViewModel

    var body: some View {
        Form {
            Section("Device") {
                LabeledContent("Name", value: Host.current().localizedName ?? "Unknown")
                LabeledContent("UUID", value: engine.deviceUUID)
            }
            Section("Fingerprint") {
                Text(engine.fingerprint)
                    .font(.system(.body, design: .monospaced))
                    .textSelection(.enabled)
            }
        }
        .formStyle(.grouped)
        .padding()
    }
}

struct TrustedDevicesTab: View {
    @ObservedObject var engine: EngineViewModel

    var body: some View {
        VStack {
            if engine.trustedDevices.isEmpty {
                VStack(spacing: 8) {
                    Label("No Trusted Devices", systemImage: "person.2.slash")
                        .font(.title2)
                        .foregroundColor(.secondary)
                    Text("Devices you exchange files with will appear here")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                List(engine.trustedDevices) { device in
                    HStack {
                        Image(systemName: device.platformIcon)
                        VStack(alignment: .leading) {
                            Text(device.name).fontWeight(.medium)
                            Text(device.fingerprint.prefix(16) + "...")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                        Spacer()
                        Menu {
                            Button("Trust") { engine.setTrust(device.id, level: .trusted) }
                            Button("Favorite") { engine.setTrust(device.id, level: .favorite) }
                            Divider()
                            Button("Block", role: .destructive) { engine.setTrust(device.id, level: .blocked) }
                        } label: {
                            Text(device.trustLabel)
                                .font(.caption)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(device.trustColor.opacity(0.15))
                                .clipShape(Capsule())
                        }
                        .menuStyle(.borderlessButton)
                    }
                }
            }
        }
        .padding()
    }
}

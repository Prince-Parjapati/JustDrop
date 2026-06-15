# JustDrop

JustDrop is a fast, server-less file transfer system designed for sharing files directly between macOS and Android over local networks.

It uses mDNS for peer discovery and establishes direct, end-to-end encrypted TCP connections. The core networking and cryptography engine is written entirely in Rust, providing minimal memory overhead and fast zero-copy transfers where supported.

## Features

- **Local network only** — no cloud, no servers, no accounts
- **End-to-end encryption** via the Noise protocol framework
- **mDNS device discovery** — peers appear automatically on the same Wi-Fi
- **Quick Settings tile** on Android — toggle JustDrop on/off from the notification shade
- **Menu bar icon** on macOS — toggle on/off from the top menu bar
- **Native Share Sheet** on Android — share any file directly to a nearby device
- **Accept / Reject popups** — get notified of incoming transfers with one-tap response
- **Progress notifications** — real-time transfer progress with speed indicator
- **Completion alerts** — shows where files were saved
- **Resume interrupted transfers** — picks up where it left off
- **SHA-256 integrity verification** on every chunk
- **Received files saved to `~/JustDrop`** (macOS) or `/sdcard/JustDrop` (Android)

## Architecture

- **Core Engine (Rust)**: Handles mDNS discovery, Noise protocol encryption, file chunking, and network transport.
- **macOS App**: A menu bar application (no dock icon) with a toggle to turn sharing on/off.
- **Android Service**: Integrates into the native Share Sheet and Quick Settings panel via JNI.

## Installation

### Android
Download the latest `JustDrop-Android-*.apk` from the [Releases](../../releases) page and install it on your device.

### macOS
Download `JustDrop-macOS.zip` from the [Releases](../../releases) page and extract it.

1. Drag **JustDrop.app** into your `/Applications` folder.
2. Double-click **JustDrop** in your Applications folder to launch it.
3. A **↔ icon** will appear in your **menu bar** (top-right corner).

> **If macOS shows a security warning:**
> Go to **System Settings → Privacy & Security** and click **"Open Anyway"**.

## Usage

### macOS
- Click the **↔** icon in the menu bar to open the JustDrop menu
- Click **"Turn On JustDrop"** to start accepting files
- Click **"Turn Off JustDrop"** to stop
- Click **"Quit JustDrop"** to close the app entirely

### Android
- Pull down the notification shade → tap the **JustDrop** Quick Settings tile to turn on
- To send files: tap **Share** in any app → select **JustDrop** → pick a nearby device
- Accept/reject incoming files via notification popups

## Uninstallation

### Android
Go to **Settings → Apps → JustDrop → Uninstall**, or long-press the app icon and tap "Uninstall".

Your received files in `/sdcard/JustDrop` will not be deleted automatically.

### macOS
1. Quit JustDrop from the menu bar icon (**Quit JustDrop**).
2. Open your `/Applications` folder and drag **JustDrop.app** to the Trash.
3. Empty the Trash.

Your received files in `~/JustDrop` will not be deleted automatically. Remove that folder manually:
```bash
rm -rf ~/JustDrop
```

## Building from source

### Prerequisites
- [Rust](https://rustup.rs/) (stable)
- Xcode Command Line Tools (for macOS builds)
- Android Studio / Android NDK (for Android builds)
- `cargo-ndk` plugin (`cargo install cargo-ndk`)

### Build macOS App
```bash
# Build the Rust FFI static library
cargo build --release -p justdrop-ffi

# Build the macOS SwiftUI App using Swift Package Manager
cd platform/macos/JustDrop
swift build -c release
```

### Build Android Libraries & APK
1. Compile the native JNI libraries for Android targets:
   ```bash
   cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o ./platform/android/app/src/main/jniLibs build --release -p justdrop-ffi
   ```
2. Build the Android app using Gradle:
   ```bash
   cd platform/android
   ./gradlew assembleRelease
   ```
The output APK will be in `platform/android/app/build/outputs/apk/release/`.

## Contributing
Contributions are welcome. Feel free to open issues or submit pull requests.

## License
MIT

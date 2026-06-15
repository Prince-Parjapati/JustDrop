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
- **macOS Daemon**: A background service with a menu bar icon for toggling on/off.
- **Android Service**: Integrates into the native Share Sheet and Quick Settings panel via JNI.

## Installation

### Android
Download the latest `JustDrop-Android.apk` from the [Releases](../../releases) page and install it on your device.

### macOS
Download `JustDrop-macOS.zip` from the [Releases](../../releases) page. Extract it and run the included `install.sh` script:
```bash
unzip JustDrop-macOS.zip
chmod +x install.sh
./install.sh
```

## Uninstallation

### Android
Go to **Settings → Apps → JustDrop → Uninstall**, or long-press the app icon and tap "Uninstall".

Your received files in `/sdcard/JustDrop` will not be deleted automatically. Remove that folder manually if you no longer need those files.

### macOS
Run the included `uninstall.sh` script:
```bash
chmod +x uninstall.sh
./uninstall.sh
```

This will:
1. Stop the background daemon
2. Remove the LaunchAgent (auto-start on login)
3. Remove the binary from `/usr/local/bin`
4. Remove configuration and encryption keys
5. Remove log files

Your received files in `~/JustDrop` will not be deleted automatically. Remove that folder manually if you no longer need those files.

## Building from source

### Prerequisites
- [Rust](https://rustup.rs/) (stable)
- Android Studio / Android NDK (for Android builds)
- `cargo-ndk` plugin (`cargo install cargo-ndk`)

### Build macOS Daemon
```bash
cargo build --release -p justdrop-daemon
```
The binary will be output to `target/release/justdrop`.

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

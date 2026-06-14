
# RustDrop

RustDrop is a fast, server-less file transfer system designed for sharing files directly between macOS and Android over local networks. 

It uses mDNS for peer discovery and establishes direct, end-to-end encrypted TCP connections. The core networking and cryptography engine is written entirely in Rust, providing minimal memory overhead and fast zero-copy transfers where supported.

## Architecture

- **Core Engine (Rust)**: Handles mDNS discovery, TLS encryption (Noise protocol), file chunking, and network transport.
- **macOS Daemon**: A background service that integrates with the native macOS Share Extension.
- **Android Service**: Integrates into the native Android Share Sheet via JNI.

## Installation

### Android
Download the latest `RustDrop-Android.apk` from the [Releases](../../releases) page and install it on your device.

### macOS
Download `RustDrop-macOS.zip` from the [Releases](../../releases) page. Extract it and run the included `install.sh` script to set up the background daemon automatically.

## Building from source

### Prerequisites
- [Rust](https://rustup.rs/) (stable)
- Android Studio / Android NDK (for Android builds)
- `cargo-ndk` plugin (`cargo install cargo-ndk`)

### Build macOS Daemon
```bash
cargo build --release -p rustdrop-daemon
```
The binary will be output to `target/release/rustdrop-macos-daemon`.

### Build Android Libraries & APK
1. Compile the native JNI libraries for Android targets:
   ```bash
   cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o ./platform/android/app/src/main/jniLibs build --release -p rustdrop-ffi
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

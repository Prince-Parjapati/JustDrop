// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "JustDrop",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "JustDrop", targets: ["JustDrop"]),
    ],
    targets: [
        .executableTarget(
            name: "JustDrop",
            path: "Sources",
            linkerSettings: [
                .linkedLibrary("justdrop_ffi"),
                .unsafeFlags(["-L../../../target/release"]),
            ]
        ),
    ]
)

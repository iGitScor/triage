// swift-tools-version: 6.0
import PackageDescription

// Swift 6 language mode everywhere (the tools version's default): strict concurrency checking.

let package = Package(
    name: "Remora",
    platforms: [.macOS(.v14)],
    targets: [
        .target(name: "RemoraCore"),
        .target(name: "RemoraPlugins", dependencies: ["RemoraCore"]),
        .executableTarget(name: "Remora", dependencies: ["RemoraCore", "RemoraPlugins"]),
        .testTarget(name: "RemoraCoreTests", dependencies: ["RemoraCore"]),
        .testTarget(name: "RemoraPluginsTests", dependencies: ["RemoraPlugins"]),
        // The app layer (inbox model, stores, Keychain vault, policy) on a temporary folder and stand-ins.
        .testTarget(name: "RemoraTests", dependencies: ["Remora"]),
    ]
)

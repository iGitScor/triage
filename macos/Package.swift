// swift-tools-version: 6.0
import PackageDescription

let settings: [SwiftSetting] = [.swiftLanguageMode(.v5)]

let package = Package(
    name: "Remora",
    platforms: [.macOS(.v14)],
    targets: [
        .target(name: "RemoraCore", swiftSettings: settings),
        .target(name: "RemoraPlugins", dependencies: ["RemoraCore"], swiftSettings: settings),
        .executableTarget(name: "Remora", dependencies: ["RemoraCore", "RemoraPlugins"], swiftSettings: settings),
        .testTarget(name: "RemoraCoreTests", dependencies: ["RemoraCore"], swiftSettings: settings),
        .testTarget(name: "RemoraPluginsTests", dependencies: ["RemoraPlugins"], swiftSettings: settings),
    ]
)

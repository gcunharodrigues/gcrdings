// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "FoundationHelper",
    platforms: [.macOS(.v15)],
    products: [.executable(name: "foundation-helper", targets: ["FoundationHelper"])],
    targets: [
        .executableTarget(name: "FoundationHelper"),
        .testTarget(name: "FoundationHelperTests", dependencies: ["FoundationHelper"]),
    ]
)

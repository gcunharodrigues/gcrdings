// swift-tools-version:5.10
import PackageDescription

let package = Package(
    name: "FluidAudioLocalBridge",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "FluidAudioLocalBridge", type: .static, targets: ["FluidAudioLocalBridge"]),
    ],
    dependencies: [
        .package(url: "https://github.com/FluidInference/FluidAudio.git", exact: "0.14.1"),
    ],
    targets: [
        .target(
            name: "FluidAudioLocalBridge",
            dependencies: [.product(name: "FluidAudio", package: "FluidAudio")],
            path: "swift"
        ),
    ]
)

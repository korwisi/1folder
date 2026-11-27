// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "OnefolderDesktop",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "OnefolderDesktop", targets: ["OnefolderDesktop"])
    ],
    targets: [
        .executableTarget(
            name: "OnefolderDesktop",
            linkerSettings: [
                .linkedLibrary("onefolder"),
                .unsafeFlags([
                    "-L../target/debug",
                    "-Xlinker", "-rpath",
                    "-Xlinker", "@executable_path/../../../../target/debug"
                ])
            ]
        )
    ]
)

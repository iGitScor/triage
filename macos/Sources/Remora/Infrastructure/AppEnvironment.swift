import AppKit
import RemoraCore

/// Where the inbox keeps its data and what it reaches outside itself. The app runs on the live Mac; tests on a
/// temporary folder and stand-ins, so they never read or erase your data, tokens or notifications.
@MainActor
struct AppEnvironment {
    var folder: URL
    var secrets: SecretStore
    var managed: ManagedValues
    var notifications: Notifying
    var http: HTTPClient
    /// Whether an installed app handles the link (slack://, linear://).
    var canOpen: (URL) -> Bool
    /// Opens a link; false when nothing did.
    var open: (URL) -> Bool
    /// Shows sample data and saves nothing (`--demo`).
    var demo: Bool
    /// Removes what earlier versions left in the HTTP cache.
    var removeURLCaches: () -> Void
    /// Where updates come from and how they are installed.
    var updates: UpdateSystem
    /// Whether the Mac has a network.
    var network: NetworkStatus

    static var live: AppEnvironment {
        AppEnvironment(
            folder: AppFolder.url,
            secrets: Keychain.live,
            managed: ManagedDefaults(),
            notifications: Notifier.shared,
            http: URLSessionHTTPClient(),
            canOpen: { NSWorkspace.shared.urlForApplication(toOpen: $0) != nil },
            open: { NSWorkspace.shared.open($0) },
            demo: CommandLine.arguments.contains("--demo"),
            removeURLCaches: URLCaches.remove,
            updates: LiveUpdateSystem(),
            network: NetworkMonitor()
        )
    }
}

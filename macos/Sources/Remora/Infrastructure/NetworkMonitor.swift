import Foundation
import Network

/// Tells the inbox when the Mac goes offline and comes back: the system's path monitor in the app, a switch
/// in tests.
@MainActor
protocol NetworkStatus {
    /// Calls back on the main actor with each change, and once with the current state.
    func observe(_ change: @escaping @MainActor (Bool) -> Void)
}

final class NetworkMonitor: NetworkStatus {
    private let monitor = NWPathMonitor()

    func observe(_ change: @escaping @MainActor (Bool) -> Void) {
        monitor.pathUpdateHandler = { path in
            let online = path.status == .satisfied
            Task { @MainActor in change(online) }
        }
        monitor.start(queue: DispatchQueue(label: "fr.igitscor.remora.network"))
    }
}

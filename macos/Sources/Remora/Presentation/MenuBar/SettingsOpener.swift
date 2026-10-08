import AppKit
import SwiftUI

/// Opens the Settings window from AppKit code. Only SwiftUI views can call `openSettings`,
/// so a tiny invisible view listens for the request.
@MainActor
enum SettingsOpener {
    private static let request = Notification.Name("RemoraOpenSettings")
    private static var window: NSWindow?

    static func install() {
        let window = NSWindow(contentRect: .zero, styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: Listener())
        window.isReleasedWhenClosed = false
        self.window = window
    }

    static func open() {
        NSApp.activate(ignoringOtherApps: true)
        NotificationCenter.default.post(name: request, object: nil)
    }

    private struct Listener: View {
        @Environment(\.openSettings) private var openSettings

        var body: some View {
            Color.clear
                .frame(width: 0, height: 0)
                .onReceive(NotificationCenter.default.publisher(for: SettingsOpener.request)) { _ in openSettings() }
        }
    }
}

import AppKit
import SwiftUI

@main
struct RemoraApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @State private var model = InboxModel.shared

    var body: some Scene {
        Settings {
            SettingsView().environment(model)
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow?
    private var statusItem: StatusItemController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        Fonts.register()
        if CommandLine.arguments.contains("--dark") { NSApp.appearance = NSAppearance(named: .darkAqua) }
        if CommandLine.arguments.contains("--window") || CommandLine.arguments.contains("--demo") { showWindow() }
        Notifier.shared.onOpen = { itemID in InboxModel.shared.openFromNotification(itemID) }
        Notifier.shared.onAction = { action, itemID in InboxModel.shared.perform(action, onItem: itemID) }
        Notifier.shared.activate()
        statusItem = MainActor.assumeIsolated { StatusItemController(model: .shared) }
        Task { @MainActor in InboxModel.shared.start() }
        NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didWakeNotification, object: nil, queue: .main
        ) { _ in
            Task { @MainActor in await InboxModel.shared.refresh() }
        }
    }

    /// The inbox in a regular window: handy when the menu bar is crowded, and for screenshots.
    @MainActor private func showWindow() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 400, height: 600),
            styleMask: [.titled, .closable, .fullSizeContentView],
            backing: .buffered, defer: false
        )
        window.titlebarAppearsTransparent = true
        window.title = "Remora"
        window.contentView = NSHostingView(rootView: InboxView().environment(InboxModel.shared))
        window.center()
        window.isReleasedWhenClosed = false
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        self.window = window
    }
}

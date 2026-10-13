import AppKit
import RemoraCore
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
        Notifier.shared.onOpen = { [weak self] itemID in
            // The summary of what arrived while Remora was closed opens the inbox.
            guard itemID.hasSuffix(ChangeDetector.whileClosedSuffix) else {
                return InboxModel.shared.openFromNotification(itemID)
            }
            self?.statusItem?.showInbox()
            return true
        }
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

    /// `remora://` links (IDEA-06): open the inbox (on a tab), the reminder panel filled in, or refresh. Nothing is
    /// done to an item without the user.
    func application(_ application: NSApplication, open urls: [URL]) {
        MainActor.assumeIsolated {
            for url in urls { follow(url) }
        }
    }

    @MainActor private func follow(_ url: URL) {
        switch RemoraLink(url) {
        case .open(let tab):
            InboxModel.shared.requestedTab = tab
            if let window {
                window.makeKeyAndOrderFront(nil)
                NSApp.activate(ignoringOtherApps: true)
            } else {
                NSApp.activate(ignoringOtherApps: true)
                statusItem?.showInbox()
            }
        case .remind(let text, let at): statusItem?.newReminder(text: text, at: at)
        case .refresh: Task { await InboxModel.shared.refresh(manual: true) }
        case nil: break
        }
    }

    /// Changes are saved a moment later, off the main thread: what's still pending is written before quitting.
    func applicationWillTerminate(_ notification: Notification) {
        MainActor.assumeIsolated { InboxModel.shared.flushWrites() }
    }

    /// The inbox in a regular window: handy when the menu bar is crowded, and for screenshots.
    @MainActor private func showWindow() {
        let window = NSWindow(
            contentRect: NSRect(origin: .zero, size: InboxView.size),
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

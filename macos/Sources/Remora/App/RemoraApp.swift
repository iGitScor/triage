import AppKit
import CoreSpotlight
import RemoraCore
import SwiftUI

@main
struct RemoraApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate

    var body: some Scene {
        Settings {
            SettingsView().environment(delegate.model)
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    /// The inbox of this run, handed to the menu bar, the window, Settings, notifications and the `remora` command.
    /// The delegate is made on the main thread, before the app finishes launching.
    let model = MainActor.assumeIsolated { InboxModel(environment: .live) }
    private var window: NSWindow?
    private var statusItem: StatusItemController?
    private var commands: LocalSocketServer?

    func applicationDidFinishLaunching(_ notification: Notification) {
        Fonts.register()
        let model = model
        if CommandLine.arguments.contains("--dark") { NSApp.appearance = NSAppearance(named: .darkAqua) }
        if CommandLine.arguments.contains("--window") || CommandLine.arguments.contains("--demo") { showWindow() }
        Notifier.shared.onOpen = { [weak self] itemID in
            // The summary of what arrived while Remora was closed opens the inbox.
            guard itemID.hasSuffix(ChangeDetector.whileClosedSuffix) else {
                return model.openFromNotification(itemID)
            }
            self?.statusItem?.showInbox()
            return true
        }
        Notifier.shared.onAction = { action, itemID in model.perform(action, onItem: itemID) }
        Notifier.shared.activate()
        statusItem = MainActor.assumeIsolated { StatusItemController(model: model) }
        Task { @MainActor in model.start() }
        commands = MainActor.assumeIsolated {
            CommandEndpoint.start(model: model, demo: CommandLine.arguments.contains("--demo"))
        }
        NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didWakeNotification, object: nil, queue: .main
        ) { _ in
            Task { @MainActor in await model.refresh() }
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
            model.requestedTab = tab
            showInbox()
        case .remind(let text, let at): statusItem?.newReminder(text: text, at: at)
        case .refresh: Task { await model.refresh(manual: true) }
        case nil: break
        }
    }

    @MainActor private func showInbox() {
        if let window {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
        } else {
            NSApp.activate(ignoringOtherApps: true)
            statusItem?.showInbox()
        }
    }

    /// A Spotlight hit (IDEA-38): the item opens as a click in the inbox would; one gone since, the inbox.
    func application(
        _ application: NSApplication, continue userActivity: NSUserActivity,
        restorationHandler: @escaping ([any NSUserActivityRestoring]) -> Void
    ) -> Bool {
        guard userActivity.activityType == CSSearchableItemActionType else { return false }
        let id = userActivity.userInfo?[CSSearchableItemActivityIdentifier] as? String
        MainActor.assumeIsolated {
            if let id, model.openFromNotification(id) { return }
            showInbox()
        }
        return true
    }

    /// Changes are saved a moment later, off the main thread: what's still pending is written before quitting.
    func applicationWillTerminate(_ notification: Notification) {
        MainActor.assumeIsolated { model.flushWrites() }
        commands?.stop()
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
        window.contentView = NSHostingView(rootView: InboxView().environment(model))
        window.center()
        window.isReleasedWhenClosed = false
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        self.window = window
    }
}

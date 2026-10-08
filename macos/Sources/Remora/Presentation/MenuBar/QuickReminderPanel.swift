import AppKit
import RemoraCore
import SwiftUI

/// Where the drag ends: type what to remember, Enter saves, Esc cancels.
@MainActor
final class QuickReminderPanel {
    private let model: InboxModel
    private let panel = FloatingPanel(interactive: true)
    private var resignObserver: NSObjectProtocol?
    private var outsideClickMonitor: Any?

    init(model: InboxModel) {
        self.model = model
        resignObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.didResignKeyNotification, object: panel, queue: .main
        ) { [weak self] _ in
            Task { @MainActor in self?.close() }
        }
    }

    func show(at point: NSPoint, date: Date) {
        panel.host(QuickReminderView(date: date) { [weak self] title in
            self?.model.addReminder(title, at: date)
            self?.close()
        } cancel: { [weak self] in
            self?.close()
        })
        panel.place(near: point, offset: NSPoint(x: -20, y: 10))
        panel.makeKeyAndOrderFront(nil)
        outsideClickMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
            Task { @MainActor in self?.close() }
        }
    }

    private func close() {
        panel.orderOut(nil)
        if let outsideClickMonitor { NSEvent.removeMonitor(outsideClickMonitor) }
        outsideClickMonitor = nil
    }
}

private struct QuickReminderView: View {
    let date: Date
    let save: (String) -> Void
    let cancel: () -> Void

    @State private var title = ""
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 6) {
                Image(systemName: "alarm.fill")
                    .font(.system(size: 10, weight: .bold))
                    .foregroundStyle(Myna.onAccent)
                    .frame(width: 20, height: 20)
                    .background(Myna.accent, in: Circle())
                Text(SnoozeClock().describe(date)).font(Myna.font(12.5, .semibold)).foregroundStyle(Myna.accentText)
            }
            TextField("Remind me to…", text: $title)
                .textFieldStyle(.plain)
                .font(Myna.font(15, .medium))
                .foregroundStyle(Myna.ink)
                .focused($focused)
                .onSubmit(submit)
                .onExitCommand(perform: cancel)
            Text("Enter to save · Esc to cancel").font(Myna.font(10.5)).foregroundStyle(Myna.muted)
        }
        .padding(14)
        .frame(width: 300, alignment: .leading)
        .background(Myna.surface, in: RoundedRectangle(cornerRadius: Myna.radiusLarge, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: Myna.radiusLarge, style: .continuous).strokeBorder(Myna.border))
        .preferredColorScheme(InboxModel.shared.preferences.appearance.colorScheme)
        .onAppear { focused = true }
    }

    private func submit() {
        let trimmed = title.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return cancel() }
        save(trimmed)
    }
}

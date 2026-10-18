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

    /// `text`: what a `remora://remind` link filled in, for the user to save.
    func show(at point: NSPoint, date: Date, text: String = "") {
        panel.host(
            QuickReminderView(
                date: date, title: text, colorScheme: model.preferences.appearance.colorScheme
            ) { [weak self] title, rule, until in
                self?.model.addReminder(title, at: date, repeating: rule, until: until)
                self?.close()
            } cancel: { [weak self] in
                self?.close()
            })
        panel.place(near: point, offset: NSPoint(x: -20, y: 10))
        panel.makeKeyAndOrderFront(nil)
        // Shown again while open (another drag): the previous monitor goes first.
        if let outsideClickMonitor { NSEvent.removeMonitor(outsideClickMonitor) }
        outsideClickMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) {
            [weak self] _ in
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
    /// The appearance chosen in Settings, read from the model that shows the panel.
    let colorScheme: ColorScheme?
    let save: (String, Recurrence.Rule?, Date?) -> Void
    let cancel: () -> Void

    init(
        date: Date, title: String = "", colorScheme: ColorScheme?,
        save: @escaping (String, Recurrence.Rule?, Date?) -> Void, cancel: @escaping () -> Void
    ) {
        self.date = date
        self.colorScheme = colorScheme
        self.save = save
        self.cancel = cancel
        _title = State(initialValue: title)
    }

    @State private var title = ""
    @State private var rule: Recurrence.Rule?
    @State private var until: Date?
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 6) {
                Image(systemName: "alarm.fill")
                    .font(.system(size: 10, weight: .bold))
                    .foregroundStyle(RemoraDesign.onAccent)
                    .frame(width: 20, height: 20)
                    .background(RemoraDesign.accent, in: Circle())
                Text(SnoozeClock().describe(date)).font(RemoraDesign.font(12.5, .semibold)).foregroundStyle(
                    RemoraDesign.accentText)
            }
            TextField("Remind me to…", text: $title)
                .textFieldStyle(.plain)
                .font(RemoraDesign.font(15, .medium))
                .foregroundStyle(RemoraDesign.ink)
                .focused($focused)
                .onSubmit(submit)
                .onExitCommand(perform: cancel)
            RepeatPicker(rule: $rule, until: $until, start: date)
            Text("Enter to save · Esc to cancel").font(RemoraDesign.font(10.5)).foregroundStyle(RemoraDesign.muted)
        }
        .padding(14)
        .frame(width: 300, alignment: .leading)
        .background(
            RemoraDesign.surface, in: RoundedRectangle(cornerRadius: RemoraDesign.radiusLarge, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: RemoraDesign.radiusLarge, style: .continuous).strokeBorder(
                RemoraDesign.border)
        )
        .preferredColorScheme(colorScheme)
        .onAppear { focused = true }
    }

    private func submit() {
        let trimmed = title.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return cancel() }
        guard !RepeatPicker.endsBeforeStart(rule: rule, until: until, start: date) else { return }
        save(trimmed, rule, until)
    }
}

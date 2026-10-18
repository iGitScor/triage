import RemoraCore
import RemoraPlugins
import SwiftUI

extension InboxView {
    @ViewBuilder private var updated: some View {
        if let last = model.sync.lastRefresh {
            LiveText { L("Updated %@", last.formatted(.relative(presentation: .named))) }
                .foregroundStyle(RemoraDesign.muted)
            // A list cut at its limit or a missing permission: the details are in Settings.
            let remarks = model.sync.remarks.values.flatMap { $0 }
            if !remarks.isEmpty {
                Button(action: showSettings) { Image(systemName: "info.circle") }
                    .buttonStyle(.plain)
                    .foregroundStyle(RemoraDesign.muted)
                    .help(remarks.joined(separator: "\n"))
                    .accessibilityLabel(remarks.joined(separator: " "))
                    .accessibilityHint(L("Opens Settings to see why"))
            }
        }
    }

    private var offlineText: String {
        guard let last = model.sync.lastRefresh else { return L("Offline") }
        return L("Offline · updated %@", last.formatted(.relative(presentation: .named)))
    }

    private func reconnectText(_ ids: [UUID]) -> String {
        guard ids.count == 1, let account = model.accounts.first(where: { $0.id == ids[0] }) else {
            return L("%d sources need reconnecting", ids.count)
        }
        return L(
            "%@ needs reconnecting", account.name ?? PluginRegistry.manifest(account.pluginID)?.name ?? account.pluginID
        )
    }

    /// When nothing could be read from the tools (offline at launch, every token rejected), the empty inbox says
    /// so instead of "All done!", which would look like there is nothing to do.
    var unreadState: EmptyState? {
        guard model.allItems.allSatisfy({ $0.bundle == .reminders }) else { return nil }
        switch model.sync.health {
        case .fine: return nil
        case .offline:
            return EmptyState(
                symbol: "wifi.slash", title: offlineText,
                message: "Your tools show up here when the connection is back.")
        case .reconnect(let ids):
            return EmptyState(
                symbol: "key.fill", title: reconnectText(ids),
                message: "Nothing can be read until then.", action: ("Reconnect…", showSettings))
        case .failing(let count):
            return EmptyState(
                symbol: "exclamationmark.triangle.fill",
                title: L("%d source failing", plural: "%d sources failing", count),
                message: "Nothing could be read: Settings says why.", action: ("Open Settings", showSettings))
        }
    }

    var footer: some View {
        HStack(spacing: 8) {
            switch model.sync.health {
            case .offline:
                // Not the tools' fault: one calm line, and the inbox as it was.
                Label {
                    LiveText { offlineText }
                } icon: {
                    Image(systemName: "wifi.slash")
                }
                .foregroundStyle(RemoraDesign.muted)
            case .reconnect(let ids):
                Button(action: showSettings) {
                    Label(reconnectText(ids), systemImage: "key.fill")
                        .foregroundStyle(RemoraDesign.color(for: .warning).foreground)
                }
                .buttonStyle(.plain)
                .help(ids.compactMap { model.sync.errors[$0]?.message }.joined(separator: "\n"))
                .accessibilityHint(L("Opens Settings to reconnect"))
            case .failing(let count):
                Button(action: showSettings) {
                    Label(
                        L("%d source failing", plural: "%d sources failing", count),
                        systemImage: "exclamationmark.triangle.fill"
                    )
                    .foregroundStyle(RemoraDesign.danger)
                }
                .buttonStyle(.plain)
                .help(model.sync.errors.values.map(\.message).joined(separator: "\n"))
                .accessibilityHint(L("Opens Settings to see why"))
            case .fine:
                updated
            }
            Spacer()
            Button {
                showShortcuts.toggle()
            } label: {
                Image(systemName: "keyboard")
            }
            .buttonStyle(.plain)
            .foregroundStyle(RemoraDesign.muted)
            .help("Keyboard shortcuts")
            .accessibilityLabel(L("Keyboard shortcuts"))
            .popover(isPresented: $showShortcuts, arrowEdge: .top) { ShortcutsList() }
            Button {
                picker = .newReminder
            } label: {
                Label("Reminder", systemImage: "plus")
            }
            .buttonStyle(.plain)
            .foregroundStyle(RemoraDesign.ink)
            .help("New reminder (or drag down from the menu bar icon)")
            Button("Quit") { NSApp.terminate(nil) }
                .buttonStyle(.plain)
                .foregroundStyle(RemoraDesign.muted)
        }
        .font(RemoraDesign.font(11.5))
        .padding(.horizontal, Self.gutter + 2)
        .padding(.vertical, 12)
        .background(RemoraDesign.surface)
        .overlay(alignment: .top) { RemoraDesign.line.frame(height: 1) }
    }
}

/// What Done, Mark all as done or Clear just did, with Undo (or ⌘Z) for a few seconds.
struct UndoBar: View {
    let point: InboxModel.UndoPoint
    @Environment(InboxModel.self) private var model

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "checkmark.circle.fill").foregroundStyle(RemoraDesign.accent)
            Text(point.message).font(RemoraDesign.font(12.5, .medium)).foregroundStyle(RemoraDesign.onDark)
            Spacer()
            Button(L("Undo")) { withAnimation(.snappy) { model.undo() } }
                .buttonStyle(.plain)
                .font(RemoraDesign.font(12.5, .semibold))
                .foregroundStyle(RemoraDesign.accent)
                .help(L("Undo") + " (⌘Z)")
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 9)
        // Dark in both themes: lime Undo on a light bar (dark mode's ink) could barely be read.
        .background(
            RemoraDesign.dark, in: RoundedRectangle(cornerRadius: RemoraDesign.radiusMedium, style: .continuous)
        )
        .padding(.horizontal, InboxView.gutter)
        .padding(.bottom, 6)
        .transition(.move(edge: .bottom).combined(with: .opacity))
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.updatesFrequently)
        .task(id: point.id) {
            AccessibilityNotification.Announcement(point.message).post()
            try? await Task.sleep(for: .seconds(8))
            withAnimation(.snappy) { model.forgetUndo(point.id) }
        }
    }
}

/// The inbox's keys, from the footer's keyboard button.
private struct ShortcutsList: View {
    private let rows: [(keys: String, action: String)] = [
        ("↑ ↓  J K", "Move between items"),
        ("⏎", "Open"),
        ("⌥ ⏎", "Open in browser"),
        ("E", "Mark as done"),
        ("S", "Snooze…"),
        ("P", "Pin"),
        ("⌘ F", "Search"),
        ("⌘ R", "Refresh"),
        ("⌘ Z", "Undo"),
        ("Esc", "Leave the search"),
    ]

    var body: some View {
        Grid(alignment: .leading, horizontalSpacing: 14, verticalSpacing: 6) {
            ForEach(rows, id: \.keys) { row in
                GridRow {
                    Text(row.keys).font(RemoraDesign.system(12, .semibold, design: .rounded)).foregroundStyle(
                        RemoraDesign.ink)
                    Text(L(row.action)).font(RemoraDesign.font(12.5)).foregroundStyle(RemoraDesign.inkSoft)
                }
            }
        }
        .padding(14)
    }
}

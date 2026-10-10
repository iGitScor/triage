import RemoraCore
import RemoraPlugins
import SwiftUI

struct InboxView: View {
    enum Tab: String, CaseIterable {
        case inProgress = "In progress", myTurn = "My turn", waiting = "Waiting", snoozed = "Snoozed", done = "Done"

        var title: String { L(self == .done ? "Done items" : rawValue) }
    }

    /// Roomy on purpose: more air between items makes a busy inbox feel calmer.
    static let size = CGSize(width: 460, height: 700)
    /// Side margin shared by the header, tabs, search and list.
    static let gutter: CGFloat = 18

    @Environment(InboxModel.self) private var model
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var tab = Self.launchTab

    /// `--demo --tab snoozed` opens on that tab, for screenshots.
    private static var launchTab: Tab {
        let arguments = CommandLine.arguments
        guard arguments.contains("--demo"), let index = arguments.firstIndex(of: "--tab"), index + 1 < arguments.count else { return .myTurn }
        return Tab.allCases.first { "\($0)".lowercased() == arguments[index + 1].lowercased() } ?? .myTurn
    }
    @State private var picker: TimePickerSubject?
    @State private var draft: DraftSubject?
    @State private var reviewing = false
    @State private var collapsed: Set<String> = []
    @State private var showBrief = false
    @State private var showingSummary: Set<String> = []
    @State private var expanded: Set<String> = []
    /// The item the keyboard is on (↑↓ or J K), and where typing goes: the list or the search field.
    @State private var selection: String?
    @FocusState private var focus: Focus?
    @State private var showShortcuts = false

    enum Focus: Hashable { case list, search }

    /// Long bundles show their most important items first and fold the rest.
    private let visibleInBundle = 5

    var body: some View {
        let layout = model.visibleLayout
        VStack(spacing: 0) {
            header
            HStack(spacing: 6) {
                TabSwitch(
                    tab: $tab,
                    tabs: layout.inProgress.isEmpty ? [.myTurn, .waiting] : [.inProgress, .myTurn, .waiting],
                    counts: [.inProgress: layout.inProgress.count, .myTurn: layout.actionCount, .waiting: layout.waitingItems.count]
                )
                ArchiveButton(tab: .snoozed, symbol: "moon.zzz", count: layout.snoozed.count, compact: !layout.inProgress.isEmpty, selection: $tab)
                ArchiveButton(tab: .done, symbol: "checkmark.circle", count: nil, compact: !layout.inProgress.isEmpty, selection: $tab)
            }
            .padding(.horizontal, Self.gutter)
            SearchField(text: Bindable(model).query, focus: $focus) {
                focus = .list
                if selection == nil { selection = navigationOrder(model.visibleLayout).first?.id }
            }
                .padding(.horizontal, Self.gutter)
                .padding(.vertical, 10)
            content(layout)
            if let undo = model.undoPoint { UndoBar(point: undo) }
            footer
        }
        .frame(width: Self.size.width, height: Self.size.height)
        .background(Myna.backdrop)
        .background {
            // ⌘F and ⌘R work wherever the focus is, the search field included.
            Group {
                Button("") { focus = .search }.keyboardShortcut("f", modifiers: .command)
                Button("") { Task { await model.refresh(manual: true) } }.keyboardShortcut("r", modifiers: .command)
                Button("") { withAnimation(.snappy) { model.undo() } }.keyboardShortcut("z", modifiers: .command)
                    .disabled(model.undoPoint == nil)
                // Escape closes the overlay on top, wherever the focus is.
                if isOverlaid { Button("") { closeOverlay() }.keyboardShortcut(.cancelAction) }
            }
            .frame(width: 0, height: 0)
            .opacity(0)
            .accessibilityHidden(true)
        }
        // An overlay is modal: VoiceOver doesn't reach the inbox behind it, and the dimmed backdrop, a pointer
        // shortcut, isn't read either.
        .accessibilityHidden(isOverlaid)
        .overlay {
            if let picker {
                Color.black.opacity(0.25)
                    .ignoresSafeArea()
                    .onTapGesture { self.picker = nil }
                    .accessibilityHidden(true)
                TimePicker(subject: picker) { self.picker = nil }
                    .accessibilityAddTraits(.isModal)
                    .transition(.scale(scale: 0.95).combined(with: .opacity))
            } else if reviewing {
                Color.black.opacity(0.25)
                    .ignoresSafeArea()
                    .onTapGesture { reviewing = false }
                    .accessibilityHidden(true)
                ReviewSession(snooze: { picker = .snooze($0) }) { reviewing = false }
                    .accessibilityAddTraits(.isModal)
                    .transition(.scale(scale: 0.95).combined(with: .opacity))
            } else if let draft {
                Color.black.opacity(0.25)
                    .ignoresSafeArea()
                    .onTapGesture { self.draft = nil }
                    .accessibilityHidden(true)
                DraftPanel(subject: draft, link: model.webLink(for: draft.item)) { self.draft = nil }
                    .accessibilityAddTraits(.isModal)
                    .transition(.scale(scale: 0.95).combined(with: .opacity))
            }
        }
        .animation(.snappy(duration: 0.2), value: picker?.id)
        // Reduce Motion: changes happen at once, without slides, springs or spins, everywhere in the inbox.
        .transaction { if reduceMotion { $0.animation = nil } }
        .preferredColorScheme(model.preferences.appearance.colorScheme)
        // Drawn again with the new size when it changes.
        .id(model.preferences.textSize)
        .onChange(of: model.popoverOpenings) {
            // Each opening starts fresh: no search left from last time.
            model.query = ""
            tab = layout.inProgress.isEmpty ? .myTurn : .inProgress
            selection = nil
            focus = .list
        }
        .onChange(of: tab) { selection = nil }
        .onChange(of: layout.inProgress.isEmpty) { _, empty in
            if empty, tab == .inProgress { withAnimation(.snappy) { tab = .myTurn } }
        }
        .onAppear {
            focus = .list
            if !layout.inProgress.isEmpty, Self.launchTab == .myTurn { tab = .inProgress }
            model.refreshIfStale()
            if model.brief != nil, model.preferences.wholeInboxBrief { showBrief = true }
        }
    }

    private var header: some View {
        HStack(spacing: 8) {
            RemoraMark()
            Text("Remora").font(Myna.font(18, .bold)).foregroundStyle(Myna.ink)
            Spacer()
            if model.preferences.wholeInboxBrief, model.assistantAccount != nil || model.brief != nil {
                ActionButton(label: "Brief", symbol: "sparkles", prominent: showBrief) {
                    withAnimation(.snappy) { showBrief.toggle() }
                    if showBrief { tab = .myTurn }
                }
            }
            // Spins while refreshing; with Reduce Motion it dims instead.
            IconButton(symbol: "arrow.clockwise", help: "Refresh") { Task { await model.refresh(manual: true) } }
                .modifier(Spin(active: model.isRefreshing && !reduceMotion))
                .opacity(model.isRefreshing && reduceMotion ? 0.4 : 1)
            IconButton(symbol: "gearshape", help: "Settings", action: showSettings)
        }
        .padding(.horizontal, Self.gutter)
        .padding(.top, 14)
        .padding(.bottom, 10)
    }

    @ViewBuilder private func content(_ layout: InboxLayout) -> some View {
        ScrollViewReader { proxy in
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 10) {
                switch tab {
                case .inProgress:
                    if layout.inProgress.isEmpty {
                        EmptyState(symbol: "play.circle", title: "Nothing in progress", message: "Start a reminder or an item to keep it in front of you.")
                    }
                    rows(layout.inProgress)
                case .myTurn:
                    if showBrief, model.preferences.wholeInboxBrief { BriefCard().transition(.opacity.combined(with: .move(edge: .top))) }
                    if model.accounts.isEmpty && model.allItems.isEmpty {
                        Onboarding(connect: showSettings)
                    } else if layout.myTurnItems.isEmpty {
                        EmptyState(
                            symbol: "sun.max.fill",
                            title: "All done!",
                            message: layout.waitingItems.isEmpty
                                ? "Nothing needs you right now."
                                : L("Nothing needs you right now. %d waiting on others.", layout.waitingItems.count)
                        )
                    }
                    if !layout.pinned.isEmpty {
                        section(id: "pinned", title: "Pinned", symbol: "pin.fill", items: layout.pinned)
                    }
                    groups(layout.myTurn, in: .myTurn)
                case .waiting:
                    if layout.waitingItems.isEmpty {
                        EmptyState(symbol: "hourglass", title: "Nothing waiting", message: "Your merge requests waiting on reviewers show up here.")
                    }
                    groups(layout.waiting, in: .waiting)
                case .snoozed:
                    if layout.snoozed.isEmpty {
                        EmptyState(symbol: "moon.zzz.fill", title: "No snoozed items", message: "Snooze an item to make it come back later.")
                    } else {
                        InsightCard()
                        if model.assistantAccount != nil, layout.snoozed.count >= 2, model.triageSuggestions.isEmpty, !model.isTriaging {
                            HStack {
                                Spacer()
                                ActionButton(label: "Triage with Claude", symbol: "sparkles") {
                                    Task { await model.triage() }
                                }
                            }
                        }
                        TriagePanel()
                    }
                    rows(layout.snoozed)
                case .done:
                    if layout.done.isEmpty {
                        EmptyState(symbol: "checkmark.circle.fill", title: "Nothing done yet", message: "Items you mark as done show up here until they change.")
                    } else {
                        HStack {
                            Text("\(layout.done.count) done").font(Myna.font(12.5, .medium)).foregroundStyle(Myna.muted)
                            Spacer()
                            Button { withAnimation(.snappy) { model.clear(layout.done) } } label: {
                                Label("Clear all", systemImage: "trash")
                                    .font(Myna.font(12, .semibold))
                                    .foregroundStyle(Myna.ink)
                                    .padding(.horizontal, 10)
                                    .padding(.vertical, 5)
                                    .background(Myna.card, in: Capsule())
                            }
                            .buttonStyle(.plain)
                            .help("Remove these items from the Done list. They come back if something new happens.")
                        }
                        .padding(.horizontal, 2)
                    }
                    rows(layout.done)
                }
            }
            .padding(.horizontal, Self.gutter)
            .padding(.bottom, 14)
        }
        .scrollIndicators(.never)
        .focusable()
        .focusEffectDisabled()
        .focused($focus, equals: .list)
        .onKeyPress(phases: .down) { handle($0) }
        .onChange(of: selection) { _, id in
            guard let id else { return }
            withAnimation(.snappy(duration: 0.15)) { proxy.scrollTo(id) }
        }
        }
    }

    // MARK: Keyboard

    /// The items in the order they're shown on this tab, skipping collapsed bundles and folded rows.
    private func navigationOrder(_ layout: InboxLayout) -> [InboxItem] {
        switch tab {
        case .inProgress: layout.inProgress
        case .myTurn: shown(id: "pinned", layout.pinned) + layout.myTurn.flatMap { shown(id: "\(Tab.myTurn.rawValue)/\($0.id)", $0.items) }
        case .waiting: layout.waiting.flatMap { shown(id: "\(Tab.waiting.rawValue)/\($0.id)", $0.items) }
        case .snoozed: layout.snoozed
        case .done: layout.done
        }
    }

    /// A bundle's rows as displayed: none when collapsed, the first few when long and not expanded.
    private func shown(id: String, _ items: [InboxItem]) -> [InboxItem] {
        if collapsed.contains(id) { return [] }
        let folded = items.count > visibleInBundle + 1 && !expanded.contains(id)
        return folded ? Array(items.prefix(visibleInBundle)) : items
    }

    /// ↑↓ or J K move, ⏎ opens (⌥⏎ in the browser), E done, S snooze, P pin, ⌘F search, ⌘R refresh.
    /// Any other letter starts a search, as typing in the popover did before.
    private func handle(_ press: KeyPress) -> KeyPress.Result {
        guard picker == nil, draft == nil, !reviewing else { return .ignored }
        if press.modifiers.contains(.command) {
            switch press.characters {
            case "f": focus = .search
            case "r": Task { await model.refresh(manual: true) }
            default: return .ignored
            }
            return .handled
        }
        let items = navigationOrder(model.visibleLayout)
        let index = selection.flatMap { id in items.firstIndex { $0.id == id } }
        let current = index.map { items[$0] }
        func move(_ step: Int) {
            guard !items.isEmpty else { return }
            let next = index.map { $0 + step } ?? (step > 0 ? 0 : items.count - 1)
            selection = items[min(max(next, 0), items.count - 1)].id
        }
        /// After an action that takes the item off this list, the keyboard lands on its neighbour.
        func leaving(_ action: (InboxItem) -> Void) {
            guard let current, let index else { return }
            let neighbour = items.indices.contains(index + 1) ? items[index + 1].id : (index > 0 ? items[index - 1].id : nil)
            withAnimation(.snappy) { action(current) }
            selection = navigationOrder(model.visibleLayout).contains { $0.id == current.id } ? current.id : neighbour
        }
        switch press.key {
        case .downArrow: move(1)
        case .upArrow: move(-1)
        case .return:
            guard let current else { return .ignored }
            model.open(current, inBrowser: press.modifiers.contains(.option))
        default:
            guard press.modifiers.isEmpty || press.modifiers == .shift else { return .ignored }
            switch press.characters.lowercased() {
            case "j": move(1)
            case "k": move(-1)
            case "e": leaving { model.toggleDone($0) }
            case "s": if let current { picker = .snooze(current) }
            case "p": if let current { model.togglePin(current) }
            default:
                guard let first = press.characters.first, first.isLetter || first.isNumber else { return .ignored }
                model.query += press.characters
                focus = .search
            }
        }
        return .handled
    }

    private func groups(_ groups: [InboxLayout.Group], in tab: Tab) -> some View {
        ForEach(groups) { group in
            section(id: "\(tab.rawValue)/\(group.id)", title: group.bundle.title, symbol: group.bundle.symbol, items: group.items, bundle: group.bundle)
        }
    }

    @ViewBuilder private func section(id: String, title: String, symbol: String, items: [InboxItem], bundle: InboxBundle? = nil) -> some View {
        let canSummarize = bundle != nil && model.assistantAccount != nil && model.assistantPolicy.items(items).count >= 2
        BundleHeader(
            title: title,
            symbol: symbol,
            count: items.count,
            collapsed: collapsed.contains(id),
            summaryShown: showingSummary.contains(id),
            toggle: { withAnimation(.snappy) { collapsed.formSymmetricDifference([id]) } },
            sweep: { withAnimation(.snappy) { model.sweep(items) } },
            snoozeAll: bundle != nil && items.count >= 2 ? { picker = .snoozeMany(items) } : nil,
            startSession: bundle == .reviews && items.count >= 2 ? { reviewing = true } : nil,
            summarize: canSummarize ? { toggleSummary(id: id, topic: bundle!.title, items: items) } : nil
        )
        if canSummarize, showingSummary.contains(id) {
            BundleSummaryLine(
                text: model.bundleSummaries[id].map(\.text),
                error: model.bundleSummaryErrors[id],
                loading: model.summarizing.contains(id)
            )
            .transition(.opacity.combined(with: .move(edge: .top)))
        }
        if !collapsed.contains(id) {
            let visible = shown(id: id, items)
            let folded = visible.count < items.count
            rows(visible)
            if folded {
                let rest = Array(items.dropFirst(visibleInBundle))
                let restKey = "\(id)/rest"
                OverflowHelp(
                    rest: rest,
                    summary: showingSummary.contains(restKey) ? model.bundleSummaries[restKey]?.text : nil,
                    summarizing: model.summarizing.contains(restKey),
                    showAll: { withAnimation(.snappy) { _ = expanded.insert(id) } },
                    snoozeAll: { picker = .snoozeMany(rest) },
                    summarize: model.assistantAccount == nil ? nil : {
                        toggleSummary(id: restKey, topic: L(bundle?.title ?? title), items: rest)
                    }
                )
            }
        }
    }

    private func toggleSummary(id: String, topic: String, items: [InboxItem]) {
        withAnimation(.snappy) { showingSummary.formSymmetricDifference([id]) }
        if showingSummary.contains(id) {
            Task { await model.summarize(key: id, topic: L(topic), items: items) }
        }
    }

    private func rows(_ items: [InboxItem]) -> some View {
        ForEach(items) { item in
            ItemRow(item: item, selected: selection == item.id && focus == .list, onSnooze: { picker = .snooze(item) }, onDraft: { draft = $0 })
                .id(item.id)
                .transition(.opacity.combined(with: .move(edge: .trailing)))
        }
    }

    private var isOverlaid: Bool { picker != nil || reviewing || draft != nil }

    /// The overlay on top: the time picker opens over a review session, so it closes first.
    private func closeOverlay() {
        if picker != nil { picker = nil } else if reviewing { reviewing = false } else { draft = nil }
    }

    @ViewBuilder private var updated: some View {
        if let last = model.lastRefresh {
            LiveText { L("Updated %@", last.formatted(.relative(presentation: .named))) }
                .foregroundStyle(Myna.muted)
            // A list cut at its limit or a missing permission: the details are in Settings.
            let remarks = model.remarks.values.flatMap { $0 }
            if !remarks.isEmpty {
                Button(action: showSettings) { Image(systemName: "info.circle") }
                    .buttonStyle(.plain)
                    .foregroundStyle(Myna.muted)
                    .help(remarks.joined(separator: "\n"))
                    .accessibilityLabel(remarks.joined(separator: " "))
                    .accessibilityHint(L("Opens Settings to see why"))
            }
        }
    }

    private var offlineText: String {
        guard let last = model.lastRefresh else { return L("Offline") }
        return L("Offline · updated %@", last.formatted(.relative(presentation: .named)))
    }

    private func reconnectText(_ ids: [UUID]) -> String {
        guard ids.count == 1, let account = model.accounts.first(where: { $0.id == ids[0] }) else {
            return L("%d sources need reconnecting", ids.count)
        }
        return L("%@ needs reconnecting", account.name ?? PluginRegistry.manifest(account.pluginID)?.name ?? account.pluginID)
    }

    private var footer: some View {
        HStack(spacing: 8) {
            switch model.sourcesHealth {
            case .offline:
                // Not the tools' fault: one calm line, and the inbox as it was.
                Label { LiveText { offlineText } } icon: { Image(systemName: "wifi.slash") }
                    .foregroundStyle(Myna.muted)
            case .reconnect(let ids):
                Button(action: showSettings) {
                    Label(reconnectText(ids), systemImage: "key.fill")
                        .foregroundStyle(Myna.color(for: .warning).foreground)
                }
                .buttonStyle(.plain)
                .help(ids.compactMap { model.errors[$0]?.message }.joined(separator: "\n"))
                .accessibilityHint(L("Opens Settings to reconnect"))
            case .failing(let count):
                Button(action: showSettings) {
                    Label(L("%d source failing", plural: "%d sources failing", count), systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(Myna.danger)
                }
                .buttonStyle(.plain)
                .help(model.errors.values.map(\.message).joined(separator: "\n"))
                .accessibilityHint(L("Opens Settings to see why"))
            case .fine:
                updated
            }
            Spacer()
            Button { showShortcuts.toggle() } label: { Image(systemName: "keyboard") }
                .buttonStyle(.plain)
                .foregroundStyle(Myna.muted)
                .help("Keyboard shortcuts")
                .accessibilityLabel(L("Keyboard shortcuts"))
                .popover(isPresented: $showShortcuts, arrowEdge: .top) { ShortcutsList() }
            Button { picker = .newReminder } label: { Label("Reminder", systemImage: "plus") }
                .buttonStyle(.plain)
                .foregroundStyle(Myna.ink)
                .help("New reminder (or drag down from the menu bar icon)")
            Button("Quit") { NSApp.terminate(nil) }
                .buttonStyle(.plain)
                .foregroundStyle(Myna.muted)
        }
        .font(Myna.font(11.5))
        .padding(.horizontal, Self.gutter + 2)
        .padding(.vertical, 12)
        .background(Myna.surface)
        .overlay(alignment: .top) { Myna.line.frame(height: 1) }
    }

    private func showSettings() {
        SettingsOpener.open()
    }
}

/// The tabs that matter: what you're on (while something runs), your turn and theirs.
private struct TabSwitch: View {
    @Binding var tab: InboxView.Tab
    let tabs: [InboxView.Tab]
    let counts: [InboxView.Tab: Int]

    var body: some View {
        HStack(spacing: 4) {
            ForEach(tabs, id: \.self) { value in
                let selected = tab == value
                Button {
                    withAnimation(.snappy(duration: 0.2)) { tab = value }
                } label: {
                    HStack(spacing: 6) {
                        if value == .inProgress {
                            Image(systemName: "play.fill").font(.system(size: 11, weight: .bold))
                        } else {
                            Text(value.title).font(Myna.font(13, .semibold)).lineLimit(1).fixedSize()
                        }
                        if let count = counts[value], count > 0 {
                            Text("\(count)")
                                .font(Myna.font(11, .bold))
                                .foregroundStyle(selected ? Myna.onAccent : Myna.muted)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 1)
                                .background(selected ? Myna.accent : Myna.line, in: Capsule())
                        }
                    }
                    .foregroundStyle(selected ? Myna.onDark : Myna.ink)
                    .frame(maxWidth: value == .inProgress ? nil : .infinity)
                    .padding(.horizontal, value == .inProgress ? 10 : 0)
                    .padding(.vertical, 7)
                    .background(selected ? Myna.dark : .clear, in: Capsule())
                    .contentShape(Capsule())
                }
                .buttonStyle(.plain)
                .help(value.title)
                // The In progress tab is an icon: say its name, its count, and which tab is selected.
                .accessibilityLabel(value.title)
                .accessibilityValue(counts[value].map { "\($0)" } ?? "")
                .accessibilityAddTraits(selected ? .isSelected : [])
            }
        }
        .padding(4)
        .background(Myna.card, in: Capsule())
    }
}

/// Snoozed and Done: archives you open now and then, so smaller than the main tabs.
private struct ArchiveButton: View {
    let tab: InboxView.Tab
    let symbol: String
    let count: Int?
    /// Icon (and count) only, when the tab row is crowded.
    var compact = false
    @Binding var selection: InboxView.Tab

    var body: some View {
        let selected = selection == tab
        Button {
            withAnimation(.snappy(duration: 0.2)) { selection = selected ? .myTurn : tab }
        } label: {
            HStack(spacing: 4) {
                Image(systemName: symbol).font(.system(size: 11, weight: .semibold))
                if compact {
                    if let count, count > 0 { Text("\(count)").font(Myna.font(11.5, .semibold)) }
                } else {
                    Text(count.map { $0 > 0 ? "\(tab.title) \($0)" : tab.title } ?? tab.title).font(Myna.font(11.5, .semibold)).lineLimit(1)
                }
            }
            .foregroundStyle(selected ? Myna.onDark : Myna.muted)
            .padding(.horizontal, 9)
            .padding(.vertical, 8)
            .background(selected ? Myna.dark : Myna.card, in: Capsule())
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .help(tab.title)
        // Compact, it's an icon and a number: say what it is.
        .accessibilityLabel(tab.title)
        .accessibilityValue(count.map { "\($0)" } ?? "")
        // Set both ways: with only the addition, Done reported itself selected while it wasn't.
        .accessibilityAddTraits(selected ? .isSelected : [])
        .accessibilityRemoveTraits(selected ? [] : .isSelected)
    }
}

/// What Done, Mark all as done or Clear just did, with Undo (or ⌘Z) for a few seconds.
private struct UndoBar: View {
    let point: InboxModel.UndoPoint
    @Environment(InboxModel.self) private var model

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "checkmark.circle.fill").foregroundStyle(Myna.accent)
            Text(point.message).font(Myna.font(12.5, .medium)).foregroundStyle(Myna.backdrop)
            Spacer()
            Button(L("Undo")) { withAnimation(.snappy) { model.undo() } }
                .buttonStyle(.plain)
                .font(Myna.font(12.5, .semibold))
                .foregroundStyle(Myna.accent)
                .help(L("Undo") + " (⌘Z)")
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 9)
        .background(Myna.ink, in: RoundedRectangle(cornerRadius: Myna.radiusMedium, style: .continuous))
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
                    Text(row.keys).font(.system(size: 12, weight: .semibold, design: .rounded)).foregroundStyle(Myna.ink)
                    Text(L(row.action)).font(Myna.font(12.5)).foregroundStyle(Myna.inkSoft)
                }
            }
        }
        .padding(14)
    }
}

private struct SearchField: View {
    @Binding var text: String
    var focus: FocusState<InboxView.Focus?>.Binding
    /// Esc (clearing the search) or ↓ hands the keyboard back to the list.
    let toList: () -> Void

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass").foregroundStyle(Myna.muted)
            TextField("Search", text: $text)
                .textFieldStyle(.plain)
                .focused(focus, equals: .search)
                .onKeyPress(.escape) {
                    text = ""
                    toList()
                    return .handled
                }
                .onKeyPress(.downArrow) {
                    toList()
                    return .handled
                }
            if !text.isEmpty {
                Button { text = "" } label: { Image(systemName: "xmark.circle.fill") }
                    .buttonStyle(.plain)
                    .foregroundStyle(Myna.muted)
            }
        }
        .font(Myna.font(13))
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .background(Myna.field, in: Capsule())
        .overlay(Capsule().strokeBorder(Myna.border))
    }
}

private struct BundleHeader: View {
    let title: String
    let symbol: String
    let count: Int
    let collapsed: Bool
    let summaryShown: Bool
    let toggle: () -> Void
    let sweep: () -> Void
    let snoozeAll: (() -> Void)?
    let startSession: (() -> Void)?
    let summarize: (() -> Void)?

    @State private var hovering = false
    @FocusState private var focused: Bool
    @Environment(\.accessibilityVoiceOverEnabled) private var voiceOver

    /// "To review, 3", already translated: not a key to look up.
    private var spokenTitle: String { "\(L(title)), \(count)" }

    /// The actions show on hover, and also for keyboard focus and VoiceOver, which can't hover.
    private var showsActions: Bool { hovering || focused || voiceOver }

    var body: some View {
        HStack(spacing: 8) {
            Button(action: toggle) {
                HStack(spacing: 8) {
                    Image(systemName: symbol)
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(Myna.onAccent)
                        .frame(width: 20, height: 20)
                        .background(Myna.accent, in: Circle())
                    // On one line whatever the actions take: they give way, the title doesn't.
                    Text(L(title)).font(Myna.font(13, .semibold)).foregroundStyle(Myna.ink).lineLimit(1).fixedSize()
                    Text("\(count)").font(Myna.font(12, .medium)).foregroundStyle(Myna.muted).fixedSize()
                    Image(systemName: "chevron.down")
                        .font(.system(size: 9, weight: .bold))
                        .foregroundStyle(Myna.muted)
                        .rotationEffect(.degrees(collapsed ? -90 : 0))
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .focused($focused)
            .accessibilityLabel(spokenTitle)
            .accessibilityValue(L(collapsed ? "Collapsed" : "Expanded"))
            .accessibilityActions {
                if let summarize { Button(L(summaryShown ? "Hide the summary" : "Summarize this bundle"), action: summarize) }
                if let snoozeAll { Button(L("Snooze all"), action: snoozeAll) }
                Button(L("Mark all as done"), action: sweep)
            }
            Spacer()
            if let summarize, showsActions || summaryShown {
                Button(action: summarize) {
                    Image(systemName: "sparkles")
                        .font(.system(size: 11, weight: .bold))
                        .foregroundStyle(summaryShown ? Myna.onAccent : Myna.accentText)
                        .frame(width: 22, height: 22)
                        .background(summaryShown ? Myna.accent : .clear, in: Circle())
                }
                .buttonStyle(.plain)
                .help(L(summaryShown ? "Hide the summary" : "Summarize this bundle"))
            }
            if let startSession {
                Button(action: startSession) {
                    Label("Start session", systemImage: "play.fill")
                        .font(Myna.font(11.5, .semibold))
                        .foregroundStyle(Myna.accentText)
                }
                .buttonStyle(.plain)
            }
            // What acts on the whole group, named for what it does, in one menu: the header stays one line.
            if showsActions {
                Menu {
                    if let snoozeAll { Button(L("Snooze all…"), systemImage: "moon.zzz", action: snoozeAll) }
                    Button(L("Mark all as done"), systemImage: "checkmark.circle", action: sweep)
                } label: {
                    Image(systemName: "ellipsis.circle")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(Myna.accentText)
                }
                .menuStyle(.borderlessButton)
                .menuIndicator(.hidden)
                .fixedSize()
                .help(L("Actions for the whole group"))
                .accessibilityLabel(L("Actions for the whole group"))
            }
        }
        .padding(.top, 8)
        .padding(.horizontal, 2)
        .contentShape(Rectangle())
        .onTapGesture(perform: toggle)
        .onHover { hovering = $0 }
    }
}

/// Two sentences from the assistant about one bundle.
private struct BundleSummaryLine: View {
    let text: String?
    let error: String?
    let loading: Bool

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            if loading {
                ProgressView().controlSize(.mini)
            } else {
                Image(systemName: "sparkles").font(.system(size: 10, weight: .bold)).foregroundStyle(Myna.accentText)
            }
            Group {
                if let error {
                    Text(error).foregroundStyle(Myna.danger)
                } else if let text, !loading {
                    Text(text).foregroundStyle(Myna.inkSoft)
                } else {
                    Text("Summarizing…").foregroundStyle(Myna.muted)
                }
            }
            .font(Myna.font(12))
            .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
        .padding(10)
        .background(Myna.accentSoft, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

/// What to do with the items a long bundle folds away: a local one-line summary, and two ways to deal
/// with them at once.
private struct OverflowHelp: View {
    let rest: [InboxItem]
    let summary: String?
    let summarizing: Bool
    let showAll: () -> Void
    let snoozeAll: () -> Void
    let summarize: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 6) {
                Button(action: showAll) {
                    Label(L("Show %d more", rest.count), systemImage: "chevron.down")
                        .font(Myna.font(12, .semibold))
                        .foregroundStyle(Myna.ink)
                }
                .buttonStyle(.plain)
                Spacer()
                ActionButton(label: "Snooze the rest", symbol: "moon.zzz", action: snoozeAll)
                if let summarize {
                    ActionButton(label: "Summarize", symbol: "sparkles", action: summarize)
                }
            }
            Text(digest).font(Myna.font(11.5)).foregroundStyle(Myna.muted)
            if summarizing {
                Text(L("Summarizing…")).font(Myna.font(12)).foregroundStyle(Myna.muted)
            } else if let summary {
                Label(summary, systemImage: "sparkles")
                    .font(Myna.font(12))
                    .foregroundStyle(Myna.inkSoft)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(10)
        .background(Myna.line, in: RoundedRectangle(cornerRadius: Myna.radiusMedium, style: .continuous))
    }

    /// "4 low priority · 1 overdue · oldest 26 d", computed locally.
    private var digest: String {
        var parts: [String] = []
        let low = rest.filter { $0.priority == .low }.count
        let pressing = rest.filter(Prioritizer().isPressing).count
        if pressing > 0 { parts.append(L("%d overdue or due today", pressing)) }
        if low > 0 { parts.append(L("%d low priority", low)) }
        if let oldest = rest.map(\.date).min() {
            parts.append(L("oldest %d d", max(0, Int(Date.now.timeIntervalSince(oldest) / 86_400))))
        }
        return parts.joined(separator: " · ")
    }
}

private struct EmptyState: View {
    let symbol: String
    let title: String
    let message: String

    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: symbol)
                .font(.system(size: 30, weight: .semibold))
                .foregroundStyle(Myna.onAccent)
                .frame(width: 72, height: 72)
                .background(Myna.accent, in: Circle())
            Text(L(title)).font(Myna.font(17, .semibold)).foregroundStyle(Myna.ink)
            Text(L(message)).font(Myna.font(12.5)).foregroundStyle(Myna.muted).multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 40)
    }
}

private struct Onboarding: View {
    let connect: () -> Void

    var body: some View {
        VStack(spacing: 14) {
            RemoraMark(size: 72)
            Text("One inbox for your work").font(Myna.font(18, .semibold)).foregroundStyle(Myna.ink)
            Text("Connect the places that need you: code reviews, chat mentions, tasks.")
                .font(Myna.font(12.5))
                .foregroundStyle(Myna.muted)
                .multilineTextAlignment(.center)
            HStack(spacing: 6) {
                ForEach(PluginRegistry.manifests, id: \.id) { manifest in
                    Label(manifest.isComingSoon ? L("%@ · soon", manifest.name) : manifest.name, systemImage: manifest.symbol)
                        .font(Myna.font(11.5, .medium))
                        .foregroundStyle(Myna.inkSoft)
                        .padding(.horizontal, 9)
                        .padding(.vertical, 4)
                        .background(Myna.card, in: Capsule())
                }
            }
            Button("Connect a source", action: connect).buttonStyle(PillButtonStyle())
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 36)
        .padding(.horizontal, 20)
    }
}

/// One turn a second while `active`, drawn from the clock: it stops as soon as `active` does. A `repeatForever`
/// animation kept spinning after the refresh ended.
private struct Spin: ViewModifier {
    let active: Bool

    func body(content: Content) -> some View {
        TimelineView(.animation(paused: !active)) { context in
            let turn = context.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 1)
            content.rotationEffect(.degrees(active ? turn * 360 : 0))
        }
    }
}

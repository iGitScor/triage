import RemoraCore
import RemoraPlugins
import SwiftUI

struct InboxView: View {
    enum Tab: String, CaseIterable {
        case myTurn = "My turn", waiting = "Waiting", snoozed = "Snoozed", done = "Done"

        var title: String { L(self == .done ? "Done items" : rawValue) }
    }

    @Environment(InboxModel.self) private var model
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

    /// Long bundles show their most important items first and fold the rest.
    private let visibleInBundle = 5

    var body: some View {
        let layout = model.layout
        VStack(spacing: 0) {
            header
            HStack(spacing: 6) {
                TabSwitch(tab: $tab, counts: [.myTurn: layout.actionCount, .waiting: layout.waitingItems.count])
                ArchiveButton(tab: .snoozed, symbol: "moon.zzz", count: layout.snoozed.count, selection: $tab)
                ArchiveButton(tab: .done, symbol: "checkmark.circle", count: nil, selection: $tab)
            }
            .padding(.horizontal, 14)
            SearchField(text: Bindable(model).query)
                .padding(.horizontal, 14)
                .padding(.vertical, 10)
            content(layout)
            footer
        }
        .frame(width: 400, height: 600)
        .background(Myna.backdrop)
        .overlay {
            if let picker {
                Color.black.opacity(0.25)
                    .ignoresSafeArea()
                    .onTapGesture { self.picker = nil }
                TimePicker(subject: picker) { self.picker = nil }
                    .transition(.scale(scale: 0.95).combined(with: .opacity))
            } else if reviewing {
                Color.black.opacity(0.25)
                    .ignoresSafeArea()
                    .onTapGesture { reviewing = false }
                ReviewSession(snooze: { picker = .snooze($0) }) { reviewing = false }
                    .transition(.scale(scale: 0.95).combined(with: .opacity))
            } else if let draft {
                Color.black.opacity(0.25)
                    .ignoresSafeArea()
                    .onTapGesture { self.draft = nil }
                DraftPanel(subject: draft) { self.draft = nil }
                    .transition(.scale(scale: 0.95).combined(with: .opacity))
            }
        }
        .animation(.snappy(duration: 0.2), value: picker?.id)
        .preferredColorScheme(model.preferences.appearance.colorScheme)
        .onAppear {
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
            IconButton(symbol: "arrow.clockwise", help: "Refresh") { Task { await model.refresh() } }
                .rotationEffect(.degrees(model.isRefreshing ? 360 : 0))
                .animation(model.isRefreshing ? .linear(duration: 1).repeatForever(autoreverses: false) : .default, value: model.isRefreshing)
            IconButton(symbol: "gearshape", help: "Settings", action: showSettings)
        }
        .padding(.horizontal, 14)
        .padding(.top, 14)
        .padding(.bottom, 10)
    }

    @ViewBuilder private func content(_ layout: InboxLayout) -> some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 8) {
                switch tab {
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
            .padding(.horizontal, 14)
            .padding(.bottom, 14)
        }
        .scrollIndicators(.never)
    }

    private func groups(_ groups: [InboxLayout.Group], in tab: Tab) -> some View {
        ForEach(groups) { group in
            section(id: "\(tab.rawValue)/\(group.id)", title: group.bundle.title, symbol: group.bundle.symbol, items: group.items, bundle: group.bundle)
        }
    }

    @ViewBuilder private func section(id: String, title: String, symbol: String, items: [InboxItem], bundle: InboxBundle? = nil) -> some View {
        let canSummarize = bundle != nil && model.assistantAccount != nil && items.count >= 2
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
            let folded = items.count > visibleInBundle + 1 && !expanded.contains(id)
            rows(folded ? Array(items.prefix(visibleInBundle)) : items)
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
            ItemRow(item: item, onSnooze: { picker = .snooze(item) }, onDraft: { draft = $0 })
                .transition(.opacity.combined(with: .move(edge: .trailing)))
        }
    }

    private var footer: some View {
        HStack(spacing: 8) {
            if !model.errors.isEmpty {
                Label(model.errors.count == 1 ? L("1 source failing") : L("%d sources failing", model.errors.count), systemImage: "exclamationmark.triangle.fill")
                    .foregroundStyle(Myna.danger)
                    .help(model.errors.values.joined(separator: "\n"))
                    .onTapGesture(perform: showSettings)
            } else if let last = model.lastRefresh {
                Text("Updated \(last.formatted(.relative(presentation: .named)))")
                    .foregroundStyle(Myna.muted)
            }
            Spacer()
            Button { picker = .newReminder } label: { Label("Reminder", systemImage: "plus") }
                .buttonStyle(.plain)
                .foregroundStyle(Myna.ink)
                .help("New reminder (or drag down from the menu bar icon)")
            Button("Quit") { NSApp.terminate(nil) }
                .buttonStyle(.plain)
                .foregroundStyle(Myna.muted)
        }
        .font(Myna.font(11.5))
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(Myna.surface)
        .overlay(alignment: .top) { Myna.line.frame(height: 1) }
    }

    private func showSettings() {
        SettingsOpener.open()
    }
}

/// The two tabs that matter: your turn and theirs.
private struct TabSwitch: View {
    @Binding var tab: InboxView.Tab
    let counts: [InboxView.Tab: Int]

    var body: some View {
        HStack(spacing: 4) {
            ForEach([InboxView.Tab.myTurn, .waiting], id: \.self) { value in
                let selected = tab == value
                Button {
                    withAnimation(.snappy(duration: 0.2)) { tab = value }
                } label: {
                    HStack(spacing: 6) {
                        Text(value.title).font(Myna.font(13, .semibold))
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
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 7)
                    .background(selected ? Myna.dark : .clear, in: Capsule())
                    .contentShape(Capsule())
                }
                .buttonStyle(.plain)
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
    @Binding var selection: InboxView.Tab

    var body: some View {
        let selected = selection == tab
        Button {
            withAnimation(.snappy(duration: 0.2)) { selection = selected ? .myTurn : tab }
        } label: {
            HStack(spacing: 4) {
                Image(systemName: symbol).font(.system(size: 11, weight: .semibold))
                Text(count.map { $0 > 0 ? "\(tab.title) \($0)" : tab.title } ?? tab.title).font(Myna.font(11.5, .semibold))
            }
            .foregroundStyle(selected ? Myna.onDark : Myna.muted)
            .padding(.horizontal, 9)
            .padding(.vertical, 8)
            .background(selected ? Myna.dark : Myna.card, in: Capsule())
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
    }
}

private struct SearchField: View {
    @Binding var text: String

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass").foregroundStyle(Myna.muted)
            TextField("Search", text: $text)
                .textFieldStyle(.plain)
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

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: symbol)
                .font(.system(size: 10, weight: .bold))
                .foregroundStyle(Myna.onAccent)
                .frame(width: 20, height: 20)
                .background(Myna.accent, in: Circle())
            Text(L(title)).font(Myna.font(13, .semibold)).foregroundStyle(Myna.ink)
            Text("\(count)").font(Myna.font(12, .medium)).foregroundStyle(Myna.muted)
            Image(systemName: "chevron.down")
                .font(.system(size: 9, weight: .bold))
                .foregroundStyle(Myna.muted)
                .rotationEffect(.degrees(collapsed ? -90 : 0))
            Spacer()
            if let summarize, hovering || summaryShown {
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
            if hovering, let snoozeAll {
                Button(action: snoozeAll) {
                    Label("Snooze all", systemImage: "moon.zzz")
                        .font(Myna.font(11.5, .semibold))
                        .foregroundStyle(Myna.accentText)
                }
                .buttonStyle(.plain)
            }
            if hovering {
                Button(action: sweep) {
                    Label("Sweep", systemImage: "checkmark.circle")
                        .font(Myna.font(11.5, .semibold))
                        .foregroundStyle(Myna.accentText)
                }
                .buttonStyle(.plain)
                .help("Mark all as done")
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

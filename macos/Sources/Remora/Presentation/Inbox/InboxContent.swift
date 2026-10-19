import RemoraCore
import RemoraPlugins
import SwiftUI

extension InboxView {
    @ViewBuilder func content(_ layout: InboxLayout) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 10) {
                    switch tab {
                    case .inProgress:
                        if layout.inProgress.isEmpty {
                            EmptyState(
                                symbol: "play.circle", title: "Nothing in progress",
                                message: "Start a reminder or an item to keep it in front of you.")
                        }
                        rows(layout.inProgress)
                    case .myTurn:
                        if showBrief, model.preferences.wholeInboxBrief {
                            BriefCard().transition(.opacity.combined(with: .move(edge: .top)))
                        }
                        if model.accounts.isEmpty && model.allItems.isEmpty {
                            Onboarding(connect: showSettings)
                        } else if layout.myTurnItems.isEmpty, let unread = unreadState {
                            unread
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
                            section(id: "pinned", title: "Pinned", symbol: "pin.fill", items: layout.pinned, mine: true)
                        }
                        groups(layout.myTurn, in: .myTurn)
                    case .waiting:
                        if layout.waitingItems.isEmpty {
                            EmptyState(
                                symbol: "hourglass", title: "Nothing waiting",
                                message: "What you wait on from others shows up here.")
                        }
                        groups(layout.waiting, in: .waiting)
                    case .snoozed:
                        if layout.snoozed.isEmpty {
                            EmptyState(
                                symbol: "moon.zzz.fill", title: "No snoozed items",
                                message: "Snooze an item to make it come back later.")
                        } else {
                            InsightCard()
                            if model.assistantAccount != nil, layout.snoozed.count >= 2,
                                model.assistant.triageSuggestions.isEmpty, !model.assistant.isTriaging
                            {
                                HStack {
                                    Spacer()
                                    ActionButton(label: "Triage with the assistant", symbol: "sparkles") {
                                        Task { await model.triage() }
                                    }
                                }
                            }
                            TriagePanel()
                        }
                        rows(layout.snoozed)
                    case .done:
                        if layout.done.isEmpty {
                            EmptyState(
                                symbol: "checkmark.circle.fill", title: "Nothing done yet",
                                message: "Items you mark as done show up here until they change.")
                        } else {
                            HStack {
                                Text("\(layout.done.count) done").font(RemoraDesign.font(12.5, .medium))
                                    .foregroundStyle(
                                        RemoraDesign.muted)
                                Spacer()
                                Button {
                                    withAnimation(.snappy) { model.clear(layout.done) }
                                } label: {
                                    Label("Clear all", systemImage: "trash")
                                        .font(RemoraDesign.font(12, .semibold))
                                        .foregroundStyle(RemoraDesign.ink)
                                        .padding(.horizontal, 10)
                                        .padding(.vertical, 5)
                                        .background(RemoraDesign.card, in: Capsule())
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

    private func groups(_ groups: [InboxLayout.Group], in tab: Tab) -> some View {
        ForEach(groups) { group in
            section(
                id: "\(tab.rawValue)/\(group.id)", title: group.bundle.title, symbol: group.bundle.symbol,
                items: group.items, bundle: group.bundle, mine: tab == .myTurn)
        }
    }

    @ViewBuilder private func section(
        id: String, title: String, symbol: String, items: [InboxItem], bundle: InboxBundle? = nil, mine: Bool = false
    ) -> some View {
        let cut = model.cutAccounts
        let canSummarize =
            bundle != nil && model.assistantAccount != nil && model.assistantPolicy.items(items).count >= 2
        BundleHeader(
            title: title,
            symbol: symbol,
            count: InboxLayout.headerCount(items, mine: mine, cut: items.contains { cut.contains($0.accountID) }),
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
                text: model.assistant.bundleSummaries[id].map(\.text),
                error: model.assistant.bundleSummaryErrors[id],
                loading: model.assistant.summarizing.contains(id)
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
                    summary: showingSummary.contains(restKey) ? model.assistant.bundleSummaries[restKey]?.text : nil,
                    summarizing: model.assistant.summarizing.contains(restKey),
                    showAll: { withAnimation(.snappy) { _ = expanded.insert(id) } },
                    snoozeAll: { picker = .snoozeMany(rest) },
                    summarize: model.assistantAccount == nil
                        ? nil
                        : {
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
            ItemRow(
                item: item, selected: selection == item.id && focus == .list, onSnooze: { picker = .snooze(item) },
                onDraft: { draft = $0 }
            )
            .id(item.id)
            .transition(.opacity.combined(with: .move(edge: .trailing)))
        }
    }
}

struct EmptyState: View {
    let symbol: String
    let title: String
    let message: String
    var action: (label: String, perform: () -> Void)?

    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: symbol)
                .font(.system(size: 30, weight: .semibold))
                .foregroundStyle(RemoraDesign.onAccent)
                .frame(width: 72, height: 72)
                .background(RemoraDesign.accent, in: Circle())
            Text(L(title)).font(RemoraDesign.font(17, .semibold)).foregroundStyle(RemoraDesign.ink)
            Text(L(message)).font(RemoraDesign.font(12.5)).foregroundStyle(RemoraDesign.muted).multilineTextAlignment(
                .center)
            if let action {
                Button(L(action.label), action: action.perform).buttonStyle(PillButtonStyle())
            }
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
            Text("One inbox for your work").font(RemoraDesign.font(18, .semibold)).foregroundStyle(RemoraDesign.ink)
            Text("Connect the places that need you: code reviews, chat mentions, tasks.")
                .font(RemoraDesign.font(12.5))
                .foregroundStyle(RemoraDesign.muted)
                .multilineTextAlignment(.center)
            HStack(spacing: 6) {
                ForEach(PluginRegistry.manifests, id: \.id) { manifest in
                    Label(
                        manifest.isComingSoon ? L("%@ · soon", manifest.name) : manifest.name,
                        systemImage: manifest.symbol
                    )
                    .font(RemoraDesign.font(11.5, .medium))
                    .foregroundStyle(RemoraDesign.inkSoft)
                    .padding(.horizontal, 9)
                    .padding(.vertical, 4)
                    .background(RemoraDesign.card, in: Capsule())
                }
            }
            HStack(spacing: 8) {
                Button("Connect a source", action: connect).buttonStyle(PillButtonStyle())
                // A second Remora with sample data, in a window that saves nothing: see it work before a token.
                Button("Try with sample data") { SampleData.open() }.buttonStyle(PillButtonStyle(prominent: false))
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 36)
        .padding(.horizontal, 20)
    }
}

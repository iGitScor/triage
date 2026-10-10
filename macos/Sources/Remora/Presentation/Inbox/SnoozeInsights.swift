import RemoraCore
import SwiftUI

/// One pattern about the snoozed pile, with at most two ways to act on it.
struct InsightCard: View {
    @Environment(InboxModel.self) private var model
    @State private var index = 0

    var body: some View {
        let insights = model.visibleInsights
        if !insights.isEmpty {
            let insight = insights[index % insights.count]
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    Image(systemName: symbol(insight))
                        .font(.system(size: 11, weight: .bold))
                        .foregroundStyle(Myna.onAccent)
                        .frame(width: 22, height: 22)
                        .background(Myna.accent, in: Circle())
                    Text(title(insight)).font(Myna.font(13.5, .semibold)).foregroundStyle(Myna.ink)
                    Spacer()
                    if insights.count > 1 {
                        Button {
                            index += 1
                        } label: {
                            Label("\(index % insights.count + 1)/\(insights.count)", systemImage: "chevron.right")
                                .labelStyle(.titleAndIcon)
                                .font(Myna.font(11, .semibold))
                                .foregroundStyle(Myna.muted)
                        }
                        .buttonStyle(.plain)
                        .help(L("Next suggestion"))
                    }
                }
                Text(message(insight))
                    .font(Myna.font(12))
                    .foregroundStyle(Myna.inkSoft)
                    .fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 6) {
                    ForEach(actions(insight), id: \.label) { action in
                        ActionButton(label: action.label, symbol: action.symbol, prominent: action.prominent) {
                            withAnimation(.snappy) { action.run() }
                        }
                    }
                    Spacer()
                    Button(L("Not now")) { withAnimation(.snappy) { model.dismiss(insight) } }
                        .buttonStyle(.plain)
                        .font(Myna.font(11.5))
                        .foregroundStyle(Myna.muted)
                }
            }
            .padding(12)
            .background(Myna.surface, in: RoundedRectangle(cornerRadius: Myna.radiusMedium, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: Myna.radiusMedium, style: .continuous).strokeBorder(Myna.border))
        }
    }

    private struct Action {
        var label: String
        var symbol: String
        var prominent = false
        var run: () -> Void
    }

    private func symbol(_ insight: SnoozeInsight) -> String {
        switch insight {
        case .loop: "arrow.triangle.2.circlepath"
        case .avoidance: "battery.25percent"
        case .pileUp: "square.stack.3d.up"
        case .cluster: "circle.grid.cross"
        case .stale: "leaf"
        }
    }

    private func title(_ insight: SnoozeInsight) -> String {
        switch insight {
        case .loop(_, let times): L("Snoozed %d times", times)
        case .avoidance(let context, _, _): L("You often put off %@", context)
        case .pileUp(let at, let items):
            L("%d items come back %@", items.count, at.formatted(.dateTime.weekday(.abbreviated).hour().minute()))
        case .cluster(let items): L("%d items on the same topic", items.count)
        case .stale(let items): L("%d snoozed item went quiet", plural: "%d snoozed items went quiet", items.count)
        }
    }

    private func message(_ insight: SnoozeInsight) -> String {
        switch insight {
        case .loop(let item, _):
            L("“%@” keeps coming back. Decide once: do it now, or let it go.", item.title)
        case .avoidance(_, _, let items):
            L("%@ A short slot at your best time often breaks the loop.", model.advisor.nudge(for: items[0]))
        case .pileUp:
            L("Spread them 30 minutes apart so they don’t land all at once.")
        case .cluster(let items):
            items.prefix(3).map(\.title).joined(separator: " · ")
        case .stale:
            L("Nothing happened on them for 3 weeks. Let them go?")
        }
    }

    private func actions(_ insight: SnoozeInsight) -> [Action] {
        switch insight {
        case .loop(let item, _):
            [
                Action(label: "Do it now", symbol: "bolt", prominent: true) {
                    model.unsnooze(item)
                    model.open(item)
                },
                Action(label: "Let it go", symbol: "checkmark") { model.toggleDone(item) },
            ]
        case .avoidance(_, _, let items):
            [
                Action(label: "Plan a short slot", symbol: "calendar.badge.clock", prominent: true) {
                    model.snoozeMany(items, until: model.suggestedReturn(for: .motivation), reason: .motivation)
                    model.dismiss(insight)
                }
            ]
        case .pileUp(let at, let items):
            [
                Action(label: "Spread them", symbol: "arrow.left.and.right", prominent: true) {
                    model.spread(items, from: at)
                }
            ]
        case .cluster(let items):
            [
                Action(label: "Same time for all", symbol: "clock", prominent: true) {
                    model.alignReturns(items)
                    model.dismiss(insight)
                }
            ]
        case .stale(let items):
            [Action(label: "Let them go", symbol: "checkmark", prominent: true) { model.sweep(items) }]
        }
    }
}

/// Claude's proposal for each snoozed item: tick what to apply.
struct TriagePanel: View {
    @Environment(InboxModel.self) private var model
    /// Suggestions the user ticked or unticked, away from their default (`Action.preselected`).
    @State private var toggled: Set<String> = []

    var body: some View {
        if model.isTriaging {
            Label(L("Claude is looking at your snoozed items…"), systemImage: "sparkles")
                .font(Myna.font(12))
                .foregroundStyle(Myna.muted)
                .padding(.horizontal, 2)
        } else if let error = model.triageError {
            Text(error).font(Myna.font(12)).foregroundStyle(Myna.danger)
        } else if !model.triageSuggestions.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Label(L("Claude’s suggestions"), systemImage: "sparkles")
                        .font(Myna.font(13, .semibold))
                    Spacer()
                    IconButton(symbol: "xmark", help: "Close", size: 22) { model.discardTriage() }
                }
                ForEach(model.triageSuggestions) { suggestion in
                    if let item = model.item(suggestion.id) {
                        row(suggestion, item: item)
                    }
                }
                if model.triageSuggestions.contains(where: { $0.action == .done || $0.action == .now }) {
                    Text(L("“Let it go” and “Do it now” are yours to tick."))
                        .font(Myna.font(11))
                        .foregroundStyle(Myna.onDark(opacity: 0.6))
                }
                let selected = TriageSuggestion.selection(model.triageSuggestions, toggled: toggled)
                Button {
                    withAnimation(.snappy) { model.apply(selected) }
                } label: {
                    Text(L("Apply %d change", plural: "Apply %d changes", selected.count)).frame(maxWidth: .infinity)
                }
                .buttonStyle(PillButtonStyle())
                .disabled(selected.isEmpty)
            }
            .foregroundStyle(Myna.onDark)
            .padding(12)
            .background(Myna.dark, in: RoundedRectangle(cornerRadius: Myna.radiusMedium, style: .continuous))
        }
    }

    private func row(_ suggestion: TriageSuggestion, item: InboxItem) -> some View {
        let included = TriageSuggestion.selection([suggestion], toggled: toggled).isEmpty == false
        return Button {
            guard suggestion.action != .keep else { return }
            toggled.formSymmetricDifference([suggestion.id])
        } label: {
            HStack(alignment: .top, spacing: 8) {
                Image(systemName: included ? "checkmark.circle.fill" : "circle")
                    .foregroundStyle(included ? Myna.accent : Myna.onDark(opacity: 0.4))
                VStack(alignment: .leading, spacing: 2) {
                    Text(item.title).font(Myna.font(12.5, .medium)).lineLimit(1)
                    Text("\(label(suggestion)) · \(suggestion.reason)")
                        .font(Myna.font(11.5))
                        .foregroundStyle(Myna.onDark(opacity: 0.65))
                        .lineLimit(2)
                }
                Spacer(minLength: 0)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func label(_ suggestion: TriageSuggestion) -> String {
        switch suggestion.action {
        case .keep: L("Keep")
        case .reschedule:
            L("Move to %@", suggestion.until?.formatted(.dateTime.weekday(.abbreviated).hour().minute()) ?? "")
        case .done: L("Let it go")
        case .now: L("Do it now")
        }
    }
}

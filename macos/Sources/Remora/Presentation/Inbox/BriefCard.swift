import RemoraCore
import SwiftUI

/// Claude's take on what to handle first.
struct BriefCard: View {
    @Environment(InboxModel.self) private var model

    var body: some View {
        // "Written … ago" and whether a new brief is possible follow the clock while the card is shown.
        TimelineView(.everyMinute) { _ in card }
    }

    private var card: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                Image(systemName: "sparkles")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundStyle(RemoraDesign.onAccent)
                    .frame(width: 22, height: 22)
                    .background(RemoraDesign.accent, in: Circle())
                Text("Your brief").font(RemoraDesign.font(14, .semibold))
                Spacer()
                if model.assistant.isBriefing {
                    ProgressView().controlSize(.small).tint(RemoraDesign.accent)
                } else {
                    Button {
                        Task { await model.makeBrief() }
                    } label: {
                        Image(systemName: "arrow.clockwise").font(.system(size: 11, weight: .bold))
                    }
                    .buttonStyle(.plain)
                    .disabled(model.isBriefFresh)
                    .opacity(model.isBriefFresh ? RemoraDesign.disabledOpacity : 1)
                    .help(
                        model.isBriefFresh
                            ? L("This brief is still fresh (Settings → General → Assistant)") : L("Write a new brief")
                    )
                    .accessibilityLabel(L("Write a new brief"))
                }
            }

            if let error = model.assistant.briefError {
                Text(error).font(RemoraDesign.font(12)).foregroundStyle(RemoraDesign.danger)
            } else if let brief = model.assistant.brief {
                Text(brief.summary)
                    .font(RemoraDesign.font(13))
                    .foregroundStyle(RemoraDesign.onDark(opacity: 0.85))
                    .fixedSize(horizontal: false, vertical: true)
                // Only what is still in the inbox: an item done or snoozed since leaves the list.
                let current = Dictionary(model.briefItems.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
                let focus = brief.focus.compactMap { focus in current[focus.id].map { (focus, $0) } }
                ForEach(Array(focus.enumerated()), id: \.element.0.id) { index, pair in
                    FocusRow(rank: index + 1, item: pair.1, reason: pair.0.reason)
                }
                Text(footer(for: brief))
                    .font(RemoraDesign.font(10.5))
                    .foregroundStyle(RemoraDesign.onDark(opacity: 0.5))
            } else {
                Text("The assistant is reading your inbox…").font(RemoraDesign.font(12.5)).foregroundStyle(
                    RemoraDesign.onDark(opacity: 0.7))
            }
        }
        .foregroundStyle(RemoraDesign.onDark)
        .padding(14)
        .background(RemoraDesign.dark, in: RoundedRectangle(cornerRadius: RemoraDesign.radiusLarge, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: RemoraDesign.radiusLarge, style: .continuous).strokeBorder(
                RemoraDesign.border.opacity(0.6))
        )
        .task { if model.assistant.brief == nil { await model.makeBrief() } }
    }

    private func footer(for brief: Brief) -> String {
        let written = L("Written %@", brief.createdAt.formatted(.relative(presentation: .named)))
        guard model.isBriefFresh else { return written }
        let minutes =
            Int(brief.freshUntil(cacheMinutes: model.preferences.briefCacheMinutes).timeIntervalSinceNow / 60) + 1
        return L("%@ · new one possible in %d min", written, minutes)
    }
}

private struct FocusRow: View {
    @Environment(InboxModel.self) private var model
    let rank: Int
    let item: InboxItem
    let reason: String

    @State private var hovering = false

    /// A button, not a tap: reachable from the keyboard and named for VoiceOver, as the inbox rows are.
    var body: some View {
        Button {
            model.open(item)
        } label: {
            content
        }
        .buttonStyle(.plain)
        .accessibilityLabel(L("%d. %@: %@", rank, item.title, reason))
    }

    private var content: some View {
        HStack(alignment: .top, spacing: 10) {
            Text("\(rank)")
                .font(RemoraDesign.font(11, .bold))
                .foregroundStyle(RemoraDesign.onAccent)
                .frame(width: 20, height: 20)
                .background(RemoraDesign.accent, in: Circle())
            VStack(alignment: .leading, spacing: 2) {
                Text(item.title).font(RemoraDesign.font(12.5, .medium)).lineLimit(1)
                Text(reason).font(RemoraDesign.font(11.5)).foregroundStyle(RemoraDesign.onDark(opacity: 0.6)).lineLimit(
                    2)
            }
            Spacer(minLength: 0)
        }
        .padding(8)
        .background(
            hovering ? Color.white.opacity(0.08) : .clear, in: RoundedRectangle(cornerRadius: 10, style: .continuous)
        )
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
    }
}

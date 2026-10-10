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
                    .foregroundStyle(Myna.onAccent)
                    .frame(width: 22, height: 22)
                    .background(Myna.accent, in: Circle())
                Text("Your brief").font(Myna.font(14, .semibold))
                Spacer()
                if model.isBriefing {
                    ProgressView().controlSize(.small).tint(Myna.accent)
                } else {
                    Button {
                        Task { await model.makeBrief() }
                    } label: {
                        Image(systemName: "arrow.clockwise").font(.system(size: 11, weight: .bold))
                    }
                    .buttonStyle(.plain)
                    .disabled(model.isBriefFresh)
                    .opacity(model.isBriefFresh ? 0.35 : 1)
                    .help(
                        model.isBriefFresh
                            ? L("This brief is still fresh (Settings → General → Assistant)") : L("Write a new brief"))
                }
            }

            if let error = model.briefError {
                Text(error).font(Myna.font(12)).foregroundStyle(Myna.danger)
            } else if let brief = model.brief {
                Text(brief.summary)
                    .font(Myna.font(13))
                    .foregroundStyle(Myna.onDark(opacity: 0.85))
                    .fixedSize(horizontal: false, vertical: true)
                ForEach(Array(brief.focus.enumerated()), id: \.element.id) { index, focus in
                    if let item = model.item(focus.id) {
                        FocusRow(rank: index + 1, item: item, reason: focus.reason)
                    }
                }
                Text(footer(for: brief))
                    .font(Myna.font(10.5))
                    .foregroundStyle(Myna.onDark(opacity: 0.5))
            } else {
                Text("The assistant is reading your inbox…").font(Myna.font(12.5)).foregroundStyle(
                    Myna.onDark(opacity: 0.7))
            }
        }
        .foregroundStyle(Myna.onDark)
        .padding(14)
        .background(Myna.dark, in: RoundedRectangle(cornerRadius: Myna.radiusLarge, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: Myna.radiusLarge, style: .continuous).strokeBorder(Myna.border.opacity(0.6))
        )
        .task { if model.brief == nil { await model.makeBrief() } }
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

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Text("\(rank)")
                .font(Myna.font(11, .bold))
                .foregroundStyle(Myna.onAccent)
                .frame(width: 20, height: 20)
                .background(Myna.accent, in: Circle())
            VStack(alignment: .leading, spacing: 2) {
                Text(item.title).font(Myna.font(12.5, .medium)).lineLimit(1)
                Text(reason).font(Myna.font(11.5)).foregroundStyle(Myna.onDark(opacity: 0.6)).lineLimit(2)
            }
            Spacer(minLength: 0)
        }
        .padding(8)
        .background(
            hovering ? Color.white.opacity(0.08) : .clear, in: RoundedRectangle(cornerRadius: 10, style: .continuous)
        )
        .contentShape(Rectangle())
        .onTapGesture { model.open(item) }
        .onHover { hovering = $0 }
    }
}

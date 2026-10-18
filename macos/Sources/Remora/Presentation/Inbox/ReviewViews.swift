import RemoraCore
import SwiftUI

/// Under a review request: "~6 min · 4 files · tests ✓ · Auth", expandable to the largest files.
struct ReviewPrepLine: View {
    let prep: ReviewPrep
    @State private var expanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Button {
                withAnimation(.snappy) { expanded.toggle() }
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "stopwatch").font(.system(size: 10, weight: .semibold))
                    Text(prep.summary).font(RemoraDesign.font(11.5, .medium)).lineLimit(1).fixedSize()
                    if prep.testsTouched {
                        Image(systemName: "checkmark.circle.fill")
                            .font(.system(size: 10, weight: .semibold))
                            .foregroundStyle(RemoraDesign.ok)
                            .help(L("tests"))
                    }
                    // Longer labels (French) fit fewer chips: the rest folds into "+N".
                    ViewThatFits(in: .horizontal) {
                        flags(shown: 2)
                        flags(shown: 1)
                        flags(shown: 0)
                    }
                    Spacer(minLength: 0)
                    Image(systemName: "chevron.down")
                        .font(.system(size: 9, weight: .bold))
                        .rotationEffect(.degrees(expanded ? 0 : -90))
                }
                .foregroundStyle(RemoraDesign.inkSoft)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            if expanded {
                ForEach(prep.topFiles, id: \.path) { file in
                    HStack {
                        Text(file.path).font(RemoraDesign.system(11)).lineLimit(1).truncationMode(.head)
                        Spacer()
                        if let additions = file.additions, let deletions = file.deletions {
                            Text("+\(additions) −\(deletions)").font(RemoraDesign.system(11))
                                .foregroundStyle(RemoraDesign.muted)
                        }
                    }
                }
            }
        }
        .padding(8)
        .background(RemoraDesign.line, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    private func flags(shown: Int) -> some View {
        HStack(spacing: 6) {
            ForEach(prep.flags.prefix(shown), id: \.self) { flag in
                BadgeChip(
                    badge: Badge(
                        id: BadgeID(rawValue: flag.rawValue), label: flag.title, symbol: flag.symbol,
                        tone: flag == .lockfileOnly ? .neutral : .warning))
            }
            if prep.flags.count > shown {
                Text("+\(prep.flags.count - shown)").font(RemoraDesign.font(10.5, .semibold)).foregroundStyle(
                    RemoraDesign.muted
                )
                .help(prep.flags.dropFirst(shown).map(\.title).joined(separator: ", "))
            }
        }
        .fixedSize()
    }
}

/// One review request at a time, quick wins first: open the diff, then Done, Snooze or Skip.
struct ReviewSession: View {
    @Environment(InboxModel.self) private var model
    let snooze: (InboxItem) -> Void
    /// The time picker is open over the session: the keys come back to it when the picker closes.
    var covered = false
    let dismiss: () -> Void

    @State private var queue: [InboxItem] = []
    @State private var index = 0
    @State private var started = Date.now
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label(L("Review session"), systemImage: "play.circle.fill").font(RemoraDesign.font(15, .semibold))
                Spacer()
                IconButton(symbol: "xmark", help: "Close", size: 24, action: dismiss)
            }
            if let item = current {
                Text(
                    L(
                        "%d of %d · ~%d min left", index + 1, queue.count,
                        ReviewQueue.remainingMinutes(Array(queue[index...]), pace: model.learning.reviewPace))
                )
                .font(RemoraDesign.font(11.5))
                .foregroundStyle(RemoraDesign.muted)
                VStack(alignment: .leading, spacing: 6) {
                    Text(item.shownContext).font(RemoraDesign.font(11.5, .medium)).foregroundStyle(RemoraDesign.muted)
                    Text(item.title).font(RemoraDesign.font(14, .semibold)).foregroundStyle(RemoraDesign.ink)
                    if let author = item.author {
                        Text(L("by %@", author.name)).font(RemoraDesign.font(11.5)).foregroundStyle(RemoraDesign.muted)
                    }
                    HStack(spacing: 4) { ForEach(item.badges) { BadgeChip(badge: $0) } }
                    if let prep = ReviewPrep(item, pace: model.learning.reviewPace) { ReviewPrepLine(prep: prep) }
                }
                .padding(12)
                .background(
                    RemoraDesign.card, in: RoundedRectangle(cornerRadius: RemoraDesign.radiusMedium, style: .continuous)
                )
                HStack(spacing: 6) {
                    Button {
                        model.open(diffItem(item))
                    } label: {
                        Label(L("Open diff"), systemImage: "arrow.up.right")
                    }
                    .buttonStyle(PillButtonStyle())
                    .keyboardShortcut(.defaultAction)
                    Spacer()
                    ActionButton(label: "Snooze", symbol: "moon.zzz") {
                        snooze(item)
                        advance()
                    }
                    ActionButton(label: "Skip", symbol: "forward") { advance() }
                    ActionButton(label: "Done", symbol: "checkmark", prominent: true) {
                        model.toggleDone(item)
                        advance()
                    }
                }
                Text(L("⏎ open · D done · S snooze · → skip · Esc close")).font(RemoraDesign.font(10.5))
                    .foregroundStyle(
                        RemoraDesign.muted)
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "checkmark.seal.fill").font(.system(size: 30)).foregroundStyle(
                        RemoraDesign.accentText)
                    Text(L("All reviewed")).font(RemoraDesign.font(16, .semibold))
                    Text(L("%d min", max(1, Int(Date.now.timeIntervalSince(started) / 60)))).font(RemoraDesign.font(12))
                        .foregroundStyle(RemoraDesign.muted)
                    Button(L("Close"), action: dismiss).buttonStyle(PillButtonStyle())
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 20)
            }
        }
        .padding(16)
        .background(
            RemoraDesign.surface, in: RoundedRectangle(cornerRadius: RemoraDesign.radiusLarge, style: .continuous)
        )
        .shadow(color: .black.opacity(0.2), radius: 20, y: 6)
        .padding(16)
        .focusable()
        .focused($focused)
        .focusEffectDisabled()
        .onKeyPress(characters: .init(charactersIn: "dDsS")) { press in
            guard let item = current else { return .ignored }
            if press.characters.lowercased() == "d" { model.toggleDone(item) } else { snooze(item) }
            advance()
            return .handled
        }
        .onKeyPress(.rightArrow) {
            advance()
            return .handled
        }
        .onKeyPress(.escape) {
            dismiss()
            return .handled
        }
        .onChange(of: covered) { _, covered in
            if !covered { focused = true }
        }
        .onAppear {
            let reviews = model.layout.myTurn.first { $0.bundle == .reviews }?.items ?? []
            queue = ReviewQueue.order(reviews)
            started = .now
            focused = true
        }
    }

    private var current: InboxItem? { index < queue.count ? queue[index] : nil }

    private func advance() {
        withAnimation(.snappy) { index += 1 }
    }

    /// GitHub, GitLab and Azure DevOps open straight on the changes.
    private func diffItem(_ item: InboxItem) -> InboxItem {
        var item = item
        guard let url = item.url else { return item }
        if item.pluginID == "azuredevops" {
            item.url = url.appending(queryItems: [URLQueryItem(name: "_a", value: "files")])
            return item
        }
        let suffix = item.pluginID == "gitlab" ? "diffs" : "files"
        if !url.path.hasSuffix(suffix) { item.url = url.appending(path: suffix) }
        return item
    }
}

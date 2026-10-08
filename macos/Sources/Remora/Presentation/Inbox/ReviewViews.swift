import RemoraCore
import SwiftUI

/// Under a review request: "~6 min · 4 files · tests ✓ · Auth", expandable to the largest files.
struct ReviewPrepLine: View {
    let prep: ReviewPrep
    @State private var expanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Button { withAnimation(.snappy) { expanded.toggle() } } label: {
                HStack(spacing: 6) {
                    Image(systemName: "stopwatch").font(.system(size: 10, weight: .semibold))
                    Text(prep.summary).font(Myna.font(11.5, .medium)).lineLimit(1).fixedSize()
                    if prep.testsTouched {
                        Image(systemName: "checkmark.circle.fill")
                            .font(.system(size: 10, weight: .semibold))
                            .foregroundStyle(Myna.ok)
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
                .foregroundStyle(Myna.inkSoft)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            if expanded {
                ForEach(prep.topFiles, id: \.path) { file in
                    HStack {
                        Text(file.path).font(.system(size: 11, design: .monospaced)).lineLimit(1).truncationMode(.head)
                        Spacer()
                        if let additions = file.additions, let deletions = file.deletions {
                            Text("+\(additions) −\(deletions)").font(.system(size: 11, design: .monospaced)).foregroundStyle(Myna.muted)
                        }
                    }
                }
            }
        }
        .padding(8)
        .background(Myna.line, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    private func flags(shown: Int) -> some View {
        HStack(spacing: 6) {
            ForEach(prep.flags.prefix(shown), id: \.self) { flag in
                BadgeChip(badge: Badge(id: flag.rawValue, label: flag.title, symbol: flag.symbol,
                                       tone: flag == .lockfileOnly ? .neutral : .warning))
            }
            if prep.flags.count > shown {
                Text("+\(prep.flags.count - shown)").font(Myna.font(10.5, .semibold)).foregroundStyle(Myna.muted)
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
    let dismiss: () -> Void

    @State private var queue: [InboxItem] = []
    @State private var index = 0
    @State private var started = Date.now
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label(L("Review session"), systemImage: "play.circle.fill").font(Myna.font(15, .semibold))
                Spacer()
                IconButton(symbol: "xmark", help: "Close", size: 24, action: dismiss)
            }
            if let item = current {
                Text(L("%d of %d · ~%d min left", index + 1, queue.count, ReviewQueue.remainingMinutes(Array(queue[index...]))))
                    .font(Myna.font(11.5))
                    .foregroundStyle(Myna.muted)
                VStack(alignment: .leading, spacing: 6) {
                    Text(item.context).font(Myna.font(11.5, .medium)).foregroundStyle(Myna.muted)
                    Text(item.title).font(Myna.font(14, .semibold)).foregroundStyle(Myna.ink)
                    if let author = item.author { Text(L("by %@", author.name)).font(Myna.font(11.5)).foregroundStyle(Myna.muted) }
                    HStack(spacing: 4) { ForEach(item.badges) { BadgeChip(badge: $0) } }
                    if let prep = ReviewPrep(item) { ReviewPrepLine(prep: prep) }
                }
                .padding(12)
                .background(Myna.card, in: RoundedRectangle(cornerRadius: Myna.radiusMedium, style: .continuous))
                HStack(spacing: 6) {
                    Button { model.open(diffItem(item)) } label: { Label(L("Open diff"), systemImage: "arrow.up.right") }
                        .buttonStyle(PillButtonStyle())
                        .keyboardShortcut(.defaultAction)
                    Spacer()
                    ActionButton(label: "Snooze", symbol: "moon.zzz") { snooze(item); advance() }
                    ActionButton(label: "Skip", symbol: "forward") { advance() }
                    ActionButton(label: "Done", symbol: "checkmark", prominent: true) { model.toggleDone(item); advance() }
                }
                Text(L("⏎ open · D done · S snooze · → skip · Esc close")).font(Myna.font(10.5)).foregroundStyle(Myna.muted)
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "checkmark.seal.fill").font(.system(size: 30)).foregroundStyle(Myna.accentText)
                    Text(L("All reviewed")).font(Myna.font(16, .semibold))
                    Text(L("%d min", max(1, Int(Date.now.timeIntervalSince(started) / 60)))).font(Myna.font(12)).foregroundStyle(Myna.muted)
                    Button(L("Close"), action: dismiss).buttonStyle(PillButtonStyle())
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 20)
            }
        }
        .padding(16)
        .background(Myna.surface, in: RoundedRectangle(cornerRadius: Myna.radiusLarge, style: .continuous))
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
        .onKeyPress(.rightArrow) { advance(); return .handled }
        .onKeyPress(.escape) { dismiss(); return .handled }
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

    /// GitHub and GitLab open straight on the changes.
    private func diffItem(_ item: InboxItem) -> InboxItem {
        var item = item
        guard let url = item.url else { return item }
        let suffix = item.pluginID == "gitlab" ? "diffs" : "files"
        if !url.path.hasSuffix(suffix) { item.url = url.appending(path: suffix) }
        return item
    }
}

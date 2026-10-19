import RemoraCore
import SwiftUI

struct BundleHeader: View {
    let title: String
    let symbol: String
    /// "23", or "12 · 11 later" in My turn (`InboxLayout.headerCount`).
    let count: String
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
                        .foregroundStyle(RemoraDesign.onAccent)
                        .frame(width: 20, height: 20)
                        .background(RemoraDesign.accent, in: Circle())
                    // On one line whatever the actions take: they give way, the title doesn't.
                    Text(L(title)).font(RemoraDesign.font(13, .semibold)).foregroundStyle(RemoraDesign.ink).lineLimit(1)
                        .fixedSize()
                    Text(count).font(RemoraDesign.font(12, .medium)).foregroundStyle(RemoraDesign.muted)
                        .fixedSize()
                    Image(systemName: "chevron.down")
                        .font(.system(size: 9, weight: .bold))
                        .foregroundStyle(RemoraDesign.muted)
                        .rotationEffect(.degrees(collapsed ? -90 : 0))
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .focused($focused)
            .accessibilityLabel(spokenTitle)
            .accessibilityValue(L(collapsed ? "Collapsed" : "Expanded"))
            .accessibilityActions {
                if let summarize {
                    Button(L(summaryShown ? "Hide the summary" : "Summarize this bundle"), action: summarize)
                }
                if let snoozeAll { Button(L("Snooze all"), action: snoozeAll) }
                Button(L("Mark all as done"), action: sweep)
            }
            Spacer()
            if let summarize, showsActions || summaryShown {
                Button(action: summarize) {
                    Image(systemName: "sparkles")
                        .font(.system(size: 11, weight: .bold))
                        .foregroundStyle(summaryShown ? RemoraDesign.onAccent : RemoraDesign.accentText)
                        .frame(width: 22, height: 22)
                        .background(summaryShown ? RemoraDesign.accent : .clear, in: Circle())
                }
                .buttonStyle(.plain)
                .help(L(summaryShown ? "Hide the summary" : "Summarize this bundle"))
                .accessibilityLabel(L(summaryShown ? "Hide the summary" : "Summarize this bundle"))
            }
            if let startSession {
                Button(action: startSession) {
                    Label("Start session", systemImage: "play.fill")
                        .font(RemoraDesign.font(11.5, .semibold))
                        .foregroundStyle(RemoraDesign.accentText)
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
                        .foregroundStyle(RemoraDesign.accentText)
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
struct BundleSummaryLine: View {
    let text: String?
    let error: String?
    let loading: Bool

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            if loading {
                ProgressView().controlSize(.mini)
            } else {
                Image(systemName: "sparkles").font(.system(size: 10, weight: .bold)).foregroundStyle(
                    RemoraDesign.accentText)
            }
            Group {
                if let error {
                    Text(error).foregroundStyle(RemoraDesign.danger)
                } else if let text, !loading {
                    Text(text).foregroundStyle(RemoraDesign.inkSoft)
                } else {
                    Text("Summarizing…").foregroundStyle(RemoraDesign.muted)
                }
            }
            .font(RemoraDesign.font(12))
            .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
        .padding(10)
        .background(RemoraDesign.accentSoft, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

/// What to do with the items a long bundle folds away: a local one-line summary, and two ways to deal
/// with them at once.
struct OverflowHelp: View {
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
                        .font(RemoraDesign.font(12, .semibold))
                        .foregroundStyle(RemoraDesign.ink)
                }
                .buttonStyle(.plain)
                Spacer()
                ActionButton(label: "Snooze the rest", symbol: "moon.zzz", action: snoozeAll)
                if let summarize {
                    ActionButton(label: "Summarize", symbol: "sparkles", action: summarize)
                }
            }
            Text(digest).font(RemoraDesign.font(11.5)).foregroundStyle(RemoraDesign.muted)
            if summarizing {
                Text(L("Summarizing…")).font(RemoraDesign.font(12)).foregroundStyle(RemoraDesign.muted)
            } else if let summary {
                Label(summary, systemImage: "sparkles")
                    .font(RemoraDesign.font(12))
                    .foregroundStyle(RemoraDesign.inkSoft)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(10)
        .background(
            RemoraDesign.line, in: RoundedRectangle(cornerRadius: RemoraDesign.radiusMedium, style: .continuous))
    }

    /// "4 low priority · 1 overdue · oldest 26 d", computed locally.
    private var digest: String {
        var parts: [String] = []
        let low = rest.filter { $0.priority == .low }.count
        let pressing = rest.filter(Prioritizer().isPressing).count
        if pressing > 0 { parts.append(L("%d overdue or due today", pressing)) }
        if low > 0 { parts.append(L("%d low priority", low)) }
        if let oldest = rest.map(\.date).min() {
            parts.append(L("oldest %d d", max(0, Int(Date.now.timeIntervalSince(oldest) / .day))))
        }
        return parts.joined(separator: " · ")
    }
}

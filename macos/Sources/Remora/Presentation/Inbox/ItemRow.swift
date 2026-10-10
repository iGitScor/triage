import RemoraCore
import RemoraPlugins
import SwiftUI

struct ItemRow: View {
    @Environment(InboxModel.self) private var model
    let item: InboxItem
    /// The row the keyboard is on: it shows its actions as if hovered.
    var selected = false
    let onSnooze: () -> Void
    var onDraft: ((DraftSubject) -> Void)?

    @State private var hovering = false

    private var state: ItemState { model.state(of: item) }
    private var showsActions: Bool { hovering || selected }

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            leading
            VStack(alignment: .leading, spacing: 4) {
                header
                Text(item.title)
                    .font(Myna.font(13.5, .medium))
                    .foregroundStyle(Myna.ink)
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)
                if let preview = item.preview {
                    Text(preview)
                        .font(Myna.font(12))
                        .foregroundStyle(Myna.muted)
                        .lineLimit(2)
                }
                if let note = state.snooze?.note {
                    Label(note, systemImage: "note.text")
                        .font(Myna.font(11.5))
                        .foregroundStyle(Myna.inkSoft)
                }
                footer
                if item.bundle == .reviews, let prep = ReviewPrep(item, pace: model.reviewPace) {
                    ReviewPrepLine(prep: prep)
                }
                if let onDraft, let help = model.waitingHelp(item) {
                    WaitingHelpLine(help: help) {
                        onDraft(DraftSubject(item: item, text: model.waitingAssistant.draft(help, for: item)))
                    }
                }
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(background, in: RoundedRectangle(cornerRadius: Myna.radiusMedium, style: .continuous))
        .overlay {
            if selected {
                RoundedRectangle(cornerRadius: Myna.radiusMedium, style: .continuous).strokeBorder(Myna.selection, lineWidth: 1.5)
            }
        }
        .overlay(alignment: .topTrailing) { if showsActions { actions } }
        .contentShape(Rectangle())
        .onTapGesture { model.open(item, inBrowser: NSEvent.modifierFlags.contains(.option)) }
        .onHover { hovering = $0 }
        .contextMenu { menu }
        .swipeActions(leading: doneSwipe, trailing: snoozeSwipe)
        // One element for VoiceOver: read as a button that opens the item, with the hover and swipe actions,
        // the linked items and the draft as named actions (the buttons inside are otherwise unreachable).
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityText)
        .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
        .accessibilityAction { model.open(item) }
        .accessibilityAction(named: L(state.done == nil ? "Mark as done" : "Move to inbox")) {
            withAnimation(.snappy) { model.toggleDone(item) }
        }
        .accessibilityAction(named: L("Snooze…"), onSnooze)
        .accessibilityAction(named: L(state.pinned ? "Unpin" : "Pin")) { model.togglePin(item) }
        .accessibilityAction(named: L(state.startedAt == nil ? "Start" : "Stop")) {
            withAnimation(.snappy) { state.startedAt == nil ? model.start(item) : model.stop(item) }
        }
        .accessibilityActions {
            if item.appURL != nil, item.url != nil {
                Button(L("Open in browser")) { model.open(item, inBrowser: true) }
            }
            if state.snooze != nil {
                Button(L("Unsnooze")) { model.unsnooze(item) }
            }
            ForEach(model.linkedItems(item)) { other in
                Button(L("Linked: %@", other.title)) { model.open(other) }
            }
            if let onDraft, let help = model.waitingHelp(item) {
                Button(Self.draftLabel(help)) {
                    onDraft(DraftSubject(item: item, text: model.waitingAssistant.draft(help, for: item)))
                }
            }
        }
    }

    private static func draftLabel(_ help: WaitingAssistant.Help) -> String {
        switch help {
        case .suggestReviewers: L("Ask for review")
        case .nudge: L("Draft a nudge")
        }
    }

    /// What VoiceOver reads: where, what, who, when, then the item's status and badges.
    private var accessibilityText: String {
        var parts = [model.accountLabel(for: item), item.shownContext, item.title, item.preview, item.author?.name, item.date.shortRelative]
        if state.pinned { parts.append(L("Pinned")) }
        if let startedAt = state.startedAt { parts.append(L("Started %@", startedAt.formatted(.relative(presentation: .named)))) }
        if state.remindedAt != nil { parts.append(L("Reminder")) }
        if let snooze = state.snooze {
            parts.append(L("Snoozed until %@", snooze.until.formatted(.dateTime.weekday(.wide).hour().minute())))
        }
        parts += item.badges.map(\.label)
        return parts.compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: ", ")
    }

    private var doneSwipe: SwipeAction {
        let isDone = state.done != nil
        return SwipeAction(
            label: isDone ? "Inbox" : "Done",
            symbol: isDone ? "tray.and.arrow.up" : "checkmark",
            tint: Myna.accent,
            foreground: Myna.onAccent,
            dismisses: true
        ) { withAnimation(.snappy) { model.toggleDone(item) } }
    }

    private var snoozeSwipe: SwipeAction {
        SwipeAction(label: "Snooze", symbol: "moon.zzz", tint: Myna.dark, foreground: Myna.onDark, dismisses: false, perform: onSnooze)
    }

    @ViewBuilder private var leading: some View {
        if item.bundle == .reminders {
            Image(systemName: "alarm.fill")
                .font(.system(size: 12, weight: .bold))
                .foregroundStyle(Myna.onAccent)
                .frame(width: 28, height: 28)
                .background(Myna.accent, in: Circle())
        } else {
            Avatar(person: item.author)
        }
    }

    private var header: some View {
        HStack(spacing: 6) {
            if let account = model.accountLabel(for: item) {
                Text(account)
                    .font(Myna.font(10, .semibold))
                    .foregroundStyle(Myna.inkSoft)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 1)
                    .background(Myna.line, in: Capsule())
            }
            Text(item.shownContext)
                .font(Myna.font(11, .medium))
                .foregroundStyle(Myna.muted)
                .lineLimit(1)
            Spacer(minLength: 4)
            if let snooze = state.snooze {
                Label(snooze.until.formatted(.dateTime.weekday(.abbreviated).hour().minute()), systemImage: snooze.mode == .hide ? "moon.zzz" : "bell")
                    .font(Myna.font(10.5, .medium))
                    .foregroundStyle(Myna.accentText)
            } else if !showsActions {
                if let reason = model.rankingReason(item) {
                    Image(systemName: "star.fill")
                        .font(.system(size: 9, weight: .bold))
                        .foregroundStyle(Myna.accentText)
                        .help(reason)
                }
                LiveText { item.date.shortRelative }
                    .font(Myna.font(11))
                    .foregroundStyle(Myna.muted)
            }
        }
    }

    @ViewBuilder private var footer: some View {
        let badges = item.badges
        let reason = state.snooze?.reason
        let linked = model.linkedItems(item)
        let times = state.snooze == nil ? 0 : model.snoozeCount(item)
        if !badges.isEmpty || !item.participants.isEmpty || state.remindedAt != nil || state.startedAt != nil || state.pinned || reason != nil || times >= 2 || !linked.isEmpty {
            HStack(spacing: 4) {
                if let startedAt = state.startedAt {
                    BadgeChip(badge: Badge(
                        id: "started", label: L("Started %@", startedAt.formatted(.relative(presentation: .named))),
                        symbol: "play.fill", tone: .accent
                    ))
                }
                if state.remindedAt != nil {
                    BadgeChip(badge: Badge(id: "reminder", label: L("Reminder"), symbol: "bell.fill", tone: .accent))
                }
                if let reason {
                    BadgeChip(badge: Badge(id: "reason", label: reason.title, symbol: reason.symbol, tone: .neutral))
                }
                if times >= 2 {
                    BadgeChip(badge: Badge(id: "times", label: L("Snoozed %d×", times), symbol: "arrow.triangle.2.circlepath", tone: .warning))
                }
                ForEach(badges) { BadgeChip(badge: $0) }
                ForEach(linked) { other in
                    Button { model.open(other) } label: {
                        BadgeChip(badge: Badge(
                            id: "link", label: other.shownContext,
                            symbol: PluginRegistry.manifest(other.pluginID)?.symbol ?? "link", tone: .accent
                        ))
                    }
                    .buttonStyle(.plain)
                    .help(L("Linked: %@", other.title))
                }
                Spacer(minLength: 4)
                PeopleStack(people: item.participants)
            }
            .padding(.top, 2)
        }
    }

    private var actions: some View {
        HStack(spacing: 4) {
            ActionButton(label: state.pinned ? "Unpin" : "Pin", symbol: state.pinned ? "pin.slash" : "pin") {
                model.togglePin(item)
            }
            if state.startedAt == nil {
                ActionButton(label: "Start", symbol: "play") { withAnimation(.snappy) { model.start(item) } }
            } else {
                ActionButton(label: "Stop", symbol: "stop") { withAnimation(.snappy) { model.stop(item) } }
            }
            ActionButton(label: "Snooze", symbol: "moon.zzz", action: onSnooze)
            ActionButton(
                label: state.done == nil ? "Done" : "Inbox",
                symbol: state.done == nil ? "checkmark" : "tray.and.arrow.up",
                prominent: true
            ) {
                withAnimation(.snappy) { model.toggleDone(item) }
            }
        }
        .padding(4)
        .background(Myna.surface, in: Capsule())
        .shadow(color: .black.opacity(0.08), radius: 4, y: 1)
        .padding(6)
    }

    @ViewBuilder private var menu: some View {
        if let url = item.url {
            Button("Open") { model.open(item) }
        if item.appURL != nil {
            Button("Open in browser") { model.open(item, inBrowser: true) }
        }
            Button("Copy link") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(url.absoluteString, forType: .string)
            }
            Divider()
        }
        Button(L(state.pinned ? "Unpin" : "Pin")) { model.togglePin(item) }
        if state.startedAt == nil {
            Button("Start") { model.start(item) }
        } else {
            Button("Stop") { model.stop(item) }
        }
        Button("Snooze…", action: onSnooze)
        if state.snooze != nil { Button("Unsnooze") { model.unsnooze(item) } }
        Button(L(state.done == nil ? "Mark as done" : "Move to inbox")) { model.toggleDone(item) }
        if state.done != nil { Button("Clear") { model.clear([item]) } }
    }

    private var background: Color {
        if state.remindedAt != nil || state.startedAt != nil || (item.hasBadge("approved") && item.bundle == .authored) {
            return showsActions ? Myna.accentSoft.opacity(1.4) : Myna.accentSoft
        }
        return showsActions ? Myna.card2 : Myna.card
    }
}

/// Under your waiting MRs: who could review, or how long the silence has lasted, with a draft one click away.
private struct WaitingHelpLine: View {
    let help: WaitingAssistant.Help
    let draft: () -> Void

    var body: some View {
        HStack(spacing: 6) {
            switch help {
            case .suggestReviewers(let people):
                Text(L("No reviewer yet")).font(Myna.font(11.5, .medium)).foregroundStyle(Myna.inkSoft)
                PeopleStack(people: people)
                Spacer(minLength: 4)
                ActionButton(label: "Ask for review", symbol: "person.badge.plus", action: draft)
            case .nudge(let people, let days):
                Text(L("Waiting on %@ for %d d", people.map(\.name).joined(separator: ", "), days))
                    .font(Myna.font(11.5, .medium))
                    .foregroundStyle(Myna.inkSoft)
                    .lineLimit(1)
                Spacer(minLength: 4)
                ActionButton(label: "Draft a nudge", symbol: "hand.wave", action: draft)
            }
        }
        .padding(8)
        .background(Myna.accentSoft, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

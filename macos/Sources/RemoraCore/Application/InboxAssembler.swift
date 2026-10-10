import Foundation

public struct InboxLayout: Equatable, Sendable {
    public struct Group: Equatable, Sendable, Identifiable {
        public var bundle: InboxBundle
        public var items: [InboxItem]
        public var id: String { bundle.id }
    }

    /// Pinned items always count as your turn.
    /// Started by the user and not finished yet. Kept out of every other section.
    public var inProgress: [InboxItem] = []
    public var pinned: [InboxItem] = []
    /// Someone is waiting on you.
    public var myTurn: [Group] = []
    /// You are waiting on someone else.
    public var waiting: [Group] = []
    public var snoozed: [InboxItem] = []
    public var done: [InboxItem] = []

    public var groups: [Group] { myTurn + waiting }
    public var inbox: [InboxItem] { pinned + groups.flatMap(\.items) }
    public var myTurnItems: [InboxItem] { pinned + myTurn.flatMap(\.items) }
    public var waitingItems: [InboxItem] { waiting.flatMap(\.items) }
    public var inboxCount: Int { inbox.count }
    /// What the menu bar counts: your turn, without quiet "To read" and low-priority items.
    public var actionCount: Int { myTurnItems.filter(\.counts).count }

    /// Items per plugin, the plugin holding the most pressing verb first (to reply, to review, to fix…),
    /// then the busiest.
    public func countsBySource(actionableOnly: Bool) -> [(pluginID: String, count: Int)] {
        var counts: [String: Int] = [:]
        var mostPressing: [String: Int] = [:]
        for item in actionableOnly ? myTurnItems.filter(\.counts) : inbox {
            counts[item.pluginID, default: 0] += 1
            mostPressing[item.pluginID] = min(mostPressing[item.pluginID] ?? .max, item.bundle.rank)
        }
        return
            counts
            .sorted { (mostPressing[$0.key]!, -$0.value, $0.key) < (mostPressing[$1.key]!, -$1.value, $1.key) }
            .map { ($0.key, $0.value) }
    }
}

public enum Placement: Equatable, Sendable {
    case inProgress, inbox, snoozed, done, cleared
}

public struct InboxAssembler: Sendable {
    public var wakeOnActivity: Bool
    public var personal: (@Sendable (InboxItem) -> Double)?

    public init(wakeOnActivity: Bool = true, personal: (@Sendable (InboxItem) -> Double)? = nil) {
        self.wakeOnActivity = wakeOnActivity
        self.personal = personal
    }

    public func placement(of item: InboxItem, state: ItemState?, now: Date) -> Placement {
        if let expires = item.expires, expires <= now, state?.startedAt == nil, state?.pinned != true {
            return .cleared
        }
        guard let state else { return .inbox }
        if state.startedAt != nil { return .inProgress }
        if let snooze = state.snooze, snooze.mode == .hide, snooze.until > now {
            let untouched = snooze.fingerprint == item.fingerprint
            let wakes = wakeOnActivity || snooze.untilNews == true
            if untouched || !wakes { return .snoozed }
        }
        if let done = state.done, done.fingerprint == item.fingerprint {
            return done.clearedAt == nil ? .done : .cleared
        }
        return .inbox
    }

    /// Your turn when its verb says so (or, for unclassified kinds, when it needs action),
    /// or when a reminder or snooze just brought it back.
    public func isMyTurn(_ item: InboxItem, state: ItemState?) -> Bool {
        if state?.remindedAt != nil { return true }
        return item.bundle.isMine ?? item.needsAction
    }

    public func layout(
        items: [InboxItem],
        states: [String: ItemState],
        now: Date = .now,
        query: String = ""
    ) -> InboxLayout {
        var layout = InboxLayout()
        var myTurn: [InboxBundle: [InboxItem]] = [:]
        var waiting: [InboxBundle: [InboxItem]] = [:]
        let matching = items.filter { $0.matches(query) }.sorted { $0.date > $1.date }

        for item in matching {
            let state = states[item.id]
            switch placement(of: item, state: state, now: now) {
            case .inProgress: layout.inProgress.append(item)
            case .snoozed: layout.snoozed.append(item)
            case .done: layout.done.append(item)
            case .cleared: break
            case .inbox where state?.pinned == true: layout.pinned.append(item)
            case .inbox where isMyTurn(item, state: state): myTurn[item.bundle, default: []].append(item)
            case .inbox: waiting[item.bundle, default: []].append(item)
            }
        }

        layout.myTurn = groups(myTurn, now: now)
        layout.waiting = groups(waiting, now: now)
        layout.inProgress.sort { (states[$0.id]?.startedAt ?? now) < (states[$1.id]?.startedAt ?? now) }
        layout.snoozed.sort { (states[$0.id]?.snooze?.until ?? now) < (states[$1.id]?.snooze?.until ?? now) }
        layout.done.sort { (states[$0.id]?.done?.at ?? now) > (states[$1.id]?.done?.at ?? now) }
        return layout
    }

    private func groups(_ items: [InboxBundle: [InboxItem]], now: Date) -> [InboxLayout.Group] {
        items
            .map {
                InboxLayout.Group(bundle: $0.key, items: Prioritizer(now: now, personal: personal).sorted($0.value))
            }
            .sorted { ($0.bundle.rank, $0.bundle.title) < ($1.bundle.rank, $1.bundle.title) }
    }
}

extension InboxItem {
    /// Counted in the menu bar and announced on arrival.
    var counts: Bool { !bundle.isQuiet && priority != .low }

    func matches(_ query: String) -> Bool {
        let query = query.trimmingCharacters(in: .whitespaces)
        guard !query.isEmpty else { return true }
        let haystack = [title, context, author?.name ?? "", preview ?? ""]
        return haystack.contains { $0.localizedCaseInsensitiveContains(query) }
    }
}

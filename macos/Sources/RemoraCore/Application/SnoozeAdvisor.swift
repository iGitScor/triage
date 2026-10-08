import Foundation

/// One snooze, kept locally to learn your habits.
public struct SnoozeRecord: Codable, Equatable, Sendable {
    public var itemID: String
    public var pluginID: String
    public var bundleID: String
    public var context: String
    public var reason: SnoozeReason?
    public var at: Date
    public var until: Date
    /// When the item was finally marked done, if it was.
    public var doneAt: Date?

    public init(item: InboxItem, reason: SnoozeReason?, at: Date, until: Date) {
        itemID = item.id
        pluginID = item.pluginID
        bundleID = item.bundle.id
        context = item.context.contextKey
        self.reason = reason
        self.at = at
        self.until = until
    }
}

/// Something worth telling about the snoozed pile. Remora shows one at a time.
public enum SnoozeInsight: Equatable, Sendable, Identifiable {
    /// The same item keeps coming back.
    case loop(InboxItem, times: Int)
    /// You often put off this context when not feeling it.
    case avoidance(context: String, times: Int, items: [InboxItem])
    /// Too many items return at the same time.
    case pileUp(at: Date, items: [InboxItem])
    /// Several snoozed items are about the same thing.
    case cluster([InboxItem])
    /// Snoozed items that nothing happened to for weeks.
    case stale([InboxItem])

    public var id: String {
        switch self {
        case .loop(let item, _): "loop/\(item.id)"
        case .avoidance(let context, _, _): "avoidance/\(context)"
        case .pileUp(let date, _): "pileUp/\(date.timeIntervalSince1970)"
        case .cluster(let items): "cluster/" + items.map(\.id).sorted().joined(separator: ",")
        case .stale(let items): "stale/" + items.map(\.id).sorted().joined(separator: ",")
        }
    }

    public var items: [InboxItem] {
        switch self {
        case .loop(let item, _): [item]
        case .avoidance(_, _, let items), .pileUp(_, let items), .cluster(let items), .stale(let items): items
        }
    }
}

/// Tells whether two titles are about the same thing.
public protocol SimilarityModel: Sendable {
    func similar(_ a: String, _ b: String) -> Bool
}

/// Snooze suggestions and pattern detection. Pure rules over your local history.
public struct SnoozeAdvisor: Sendable {
    public var clock: SnoozeClock
    public var similarity: [any SimilarityModel]

    public init(clock: SnoozeClock = SnoozeClock(), similarity: [any SimilarityModel] = [KeywordSimilarity()]) {
        self.clock = clock
        self.similarity = similarity
    }

    private var calendar: Calendar { clock.calendar }

    // MARK: When to come back

    public func suggestedReturn(for reason: SnoozeReason, history: [SnoozeRecord], now: Date = .now) -> Date {
        switch reason {
        case .waiting:
            return morning(daysAfter: 3, of: now, hour: 9)
        case .noTime:
            let later = now.addingTimeInterval(3 * 3_600)
            return calendar.isDate(later, inSameDayAs: now) && calendar.component(.hour, from: later) < 19
                ? clock.roundedUp(later)
                : clock.tomorrowMorning(after: now)
        case .focus, .motivation:
            return next(hour: bestHour(history: history), after: now)
        case .notUrgent:
            let monday = calendar.nextDate(after: now, matching: DateComponents(weekday: 2), matchingPolicy: .nextTime) ?? now
            return morning(daysAfter: 0, of: monday, hour: 9)
        }
    }

    /// The hour you most often finish things, learned from done snoozes (9:00 until there's enough data).
    public func bestHour(history: [SnoozeRecord]) -> Int {
        let hours = history.compactMap(\.doneAt).map { calendar.component(.hour, from: $0) }.filter { (7...20).contains($0) }
        guard hours.count >= 5 else { return 9 }
        let counts = Dictionary(grouping: hours, by: { $0 }).mapValues(\.count)
        return counts.max { ($0.value, -$0.key) < ($1.value, -$1.key) }?.key ?? 9
    }

    /// What you usually pick for this repo or channel (or this kind of item), from at least three past snoozes.
    public func usualReturn(for item: InboxItem, history: [SnoozeRecord], now: Date = .now) -> Date? {
        let sameContext = history.filter { $0.context == item.context.contextKey }
        let similar = sameContext.count >= 3 ? sameContext : history.filter { $0.bundleID == item.bundle.id }
        guard similar.count >= 3 else { return nil }
        let durations = similar.map { $0.until.timeIntervalSince($0.at) }.sorted()
        let median = durations[durations.count / 2]
        let date = now.addingTimeInterval(median)
        return median >= 12 * 3_600 ? morning(daysAfter: 0, of: date, hour: 9) : clock.roundedUp(date)
    }

    /// A small, guilt-free way in when you're not feeling it.
    public func nudge(for item: InboxItem) -> String {
        if let size = item.diffSize {
            return size > 300 ? L("Do just the first step") : L("Start with 10 minutes")
        }
        if item.bundle == .reply || item.bundle == .tasks { return L("Start with 10 minutes") }
        return L("Pair it with something easy")
    }

    /// Reschedules items 30 minutes apart, starting at `start`.
    public func spread(_ items: [InboxItem], from start: Date, step: TimeInterval = 30 * 60) -> [String: Date] {
        Dictionary(uniqueKeysWithValues: items.enumerated().map { ($1.id, start.addingTimeInterval(Double($0) * step)) })
    }

    // MARK: Patterns

    public func insights(
        snoozed: [InboxItem],
        states: [String: ItemState],
        history: [SnoozeRecord],
        now: Date = .now
    ) -> [SnoozeInsight] {
        let recent = history.filter { $0.at > now.addingTimeInterval(-30 * 86_400) }
        var insights: [SnoozeInsight] = []

        let loops = snoozed
            .map { item in (item, recent.filter { $0.itemID == item.id }.count) }
            .filter { $0.1 >= 3 }
            .sorted { $0.1 > $1.1 }
        insights += loops.map { SnoozeInsight.loop($0.0, times: $0.1) }

        let avoided = Dictionary(grouping: recent.filter { $0.reason == .motivation }, by: \.context)
            .filter { $0.value.count >= 3 }
            .sorted { $0.value.count > $1.value.count }
        for (context, records) in avoided {
            let items = snoozed.filter { $0.context.contextKey == context }
            if !items.isEmpty { insights.append(.avoidance(context: context, times: records.count, items: items)) }
        }

        let byHour = Dictionary(grouping: snoozed) { item -> Date? in
            states[item.id]?.snooze.flatMap { calendar.dateInterval(of: .hour, for: $0.until)?.start }
        }
        if let pile = byHour.compactMap({ key, items in key.map { ($0, items) } })
            .filter({ $0.1.count >= 4 })
            .min(by: { $0.0 < $1.0 }) {
            insights.append(.pileUp(at: pile.0, items: pile.1.sorted { $0.date > $1.date }))
        }

        if let cluster = largestCluster(snoozed), cluster.count >= 2 {
            insights.append(.cluster(cluster))
        }

        let stale = snoozed.filter { $0.date < now.addingTimeInterval(-21 * 86_400) }
        if !stale.isEmpty { insights.append(.stale(stale)) }

        return insights
    }

    private func largestCluster(_ items: [InboxItem]) -> [InboxItem]? {
        items
            .map { seed in items.filter { $0.id == seed.id || isSimilar($0, seed) } }
            .max { $0.count < $1.count }
    }

    private func isSimilar(_ a: InboxItem, _ b: InboxItem) -> Bool {
        similarity.contains { $0.similar(a.title, b.title) }
    }

    // MARK: Helpers

    private func morning(daysAfter days: Int, of date: Date, hour: Int) -> Date {
        let day = calendar.date(byAdding: .day, value: days, to: date) ?? date
        return calendar.date(bySettingHour: hour, minute: 0, second: 0, of: day) ?? day
    }

    private func next(hour: Int, after now: Date) -> Date {
        let today = morning(daysAfter: 0, of: now, hour: hour)
        return today.timeIntervalSince(now) > 3_600 ? today : morning(daysAfter: 1, of: now, hour: hour)
    }
}

/// Same ticket key ("ENG-123"), or most significant words in common.
public struct KeywordSimilarity: SimilarityModel {
    static let stopWords: Set<String> = [
        "the", "and", "for", "with", "from", "into", "this", "that", "your", "about", "when", "what", "after", "new",
        "pour", "avec", "dans", "les", "des", "une", "sur", "est", "pas", "qui", "que", "vous", "nous", "vers",
        "la", "le", "de", "du", "au",
    ]

    public init() {}

    public func similar(_ a: String, _ b: String) -> Bool {
        let keysA = Self.ticketKeys(a), keysB = Self.ticketKeys(b)
        if !keysA.isDisjoint(with: keysB) { return true }
        let wordsA = Self.words(a), wordsB = Self.words(b)
        let shared = wordsA.intersection(wordsB).count
        let union = wordsA.union(wordsB).count
        return shared >= 2 && Double(shared) / Double(max(union, 1)) >= 0.4
    }

    static func ticketKeys(_ text: String) -> Set<String> {
        let regex = #/[A-Z][A-Z0-9]+-\d+/#
        return Set(text.matches(of: regex).map { String($0.output) })
    }

    static func words(_ text: String) -> Set<String> {
        Set(text.lowercased()
            .split { !$0.isLetter && !$0.isNumber }
            .map(String.init)
            .filter { $0.count >= 4 && !stopWords.contains($0) })
    }
}

extension String {
    /// The place without the item number: "acme/app #12" → "acme/app", "g/p !3" → "g/p", "#releases" stays.
    var contextKey: String {
        var parts = split(separator: " ")
        if let last = parts.last, last.count > 1, let first = last.first, "#!".contains(first),
           last.dropFirst().allSatisfy(\.isNumber) {
            parts.removeLast()
        }
        return parts.isEmpty ? self : parts.joined(separator: " ")
    }
}

extension InboxItem {
    /// Lines changed, read from the "+a −d" badge.
    var diffSize: Int? {
        guard let label = badges.first(where: { $0.id == "diff" })?.label else { return nil }
        let numbers = label.split { !$0.isNumber }.compactMap { Int($0) }
        return numbers.isEmpty ? nil : numbers.reduce(0, +)
    }
}

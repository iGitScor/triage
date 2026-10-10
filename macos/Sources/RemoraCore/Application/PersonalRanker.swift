import Foundation

/// What you did with an item, kept on this Mac to learn your habits.
public struct ActionRecord: Codable, Equatable, Sendable {
    public enum Outcome: String, Codable, Sendable {
        /// Opened or finished within a day of appearing.
        case quick
        /// Snoozed.
        case deferred
    }

    public var features: [String]
    public var outcome: Outcome
    public var at: Date

    public init(item: InboxItem, outcome: Outcome, at: Date = .now) {
        features = PersonalRanker.features(of: item)
        self.outcome = outcome
        self.at = at
    }
}

/// A small Naive Bayes model over your own actions: how likely you are to handle an item quickly.
/// Transparent (each score has a reason), trained instantly, and silent until it has enough data.
public struct PersonalRanker: Sendable {
    public static let minimumRecords = 20

    private var counts: [ActionRecord.Outcome: [String: Double]] = [:]
    private var totals: [ActionRecord.Outcome: Double] = [:]
    private var vocabulary: Set<String> = []
    public private(set) var isTrained = false

    public init(records: [ActionRecord]) {
        isTrained =
            records.count >= Self.minimumRecords
            && Set(records.map(\.outcome)).count == 2
        for record in records {
            totals[record.outcome, default: 0] += 1
            for feature in record.features {
                counts[record.outcome, default: [:]][feature, default: 0] += 1
                vocabulary.insert(feature)
            }
        }
    }

    /// Log-odds of handling the item quickly; 0 when untrained.
    public func score(_ item: InboxItem) -> Double {
        guard isTrained else { return 0 }
        return Self.features(of: item).reduce(prior) { $0 + contribution(of: $1) }
    }

    /// The feature that pushes the item up the most, when it clearly does.
    public func reason(_ item: InboxItem) -> String? {
        guard isTrained, score(item) > 1 else { return nil }
        let strongest = Self.features(of: item)
            .filter { !$0.hasPrefix("word:") }
            .map { ($0, contribution(of: $0)) }
            .max { $0.1 < $1.1 }
        guard let (feature, weight) = strongest, weight > 0.5 else { return nil }
        return Self.describe(feature)
    }

    private var prior: Double {
        log((totals[.quick] ?? 0) + 1) - log((totals[.deferred] ?? 0) + 1)
    }

    /// Laplace-smoothed log-likelihood ratio of one feature.
    private func contribution(of feature: String) -> Double {
        guard vocabulary.contains(feature) else { return 0 }
        let size = Double(vocabulary.count)
        let quick = ((counts[.quick]?[feature] ?? 0) + 1) / ((totals[.quick] ?? 0) + size)
        let deferred = ((counts[.deferred]?[feature] ?? 0) + 1) / ((totals[.deferred] ?? 0) + size)
        return log(quick / deferred)
    }

    static func features(of item: InboxItem) -> [String] {
        var features = ["tool:\(item.pluginID)", "verb:\(item.bundle.id)", "place:\(item.context.contextKey)"]
        if let author = item.author?.name { features.append("from:\(author)") }
        features += item.badges.map { "badge:\($0.id)" }
        features += EmbeddingSimilarity.words(LinkFinder.normalized(item.title)).prefix(6).map { "word:\($0)" }
        return features
    }

    static func describe(_ feature: String) -> String {
        let value = String(feature.drop { $0 != ":" }.dropFirst())
        switch feature.split(separator: ":").first {
        case "from": return L("You usually handle items from %@ quickly.", value)
        case "place": return L("You usually handle %@ quickly.", value)
        case "tool": return L("You usually handle %@ items quickly.", value.capitalized)
        default: return L("You usually handle items like this quickly.")
        }
    }
}

extension SnoozeAdvisor {
    /// The reason you usually give for this repo or channel: at least three snoozes, 60% the same.
    public func usualReason(for item: InboxItem, history: [SnoozeRecord]) -> SnoozeReason? {
        let reasons = history.filter { $0.context == item.context.contextKey }.compactMap(\.reason)
        guard reasons.count >= 3 else { return nil }
        let counts = Dictionary(grouping: reasons, by: { $0 }).mapValues(\.count)
        guard let (reason, count) = counts.max(by: { $0.value < $1.value }),
            Double(count) / Double(reasons.count) >= 0.6
        else {
            return nil
        }
        return reason
    }
}

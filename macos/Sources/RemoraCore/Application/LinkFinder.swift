import Foundation

/// Finds items from different tools that are about the same thing, even when nobody linked them:
/// a shared ticket key ("CE-1390"), or titles judged similar by the `SimilarityModel`s.
public struct LinkFinder: Sendable {
    public var similarity: [any SimilarityModel]
    /// At most this many links per item, most certain first.
    public var limit: Int
    /// What the similarity models said of each pair of titles at the last search: a refresh where nothing
    /// changed compares nothing again, and a new or renamed item only its own pairs. Shared by copies of the finder.
    let remembered = SimilarityMemory()

    public init(similarity: [any SimilarityModel] = [KeywordSimilarity()], limit: Int = 3) {
        self.similarity = similarity
        self.limit = limit
    }

    /// Item ID → linked item IDs.
    public func links(_ items: [InboxItem]) -> [String: [String]] {
        let candidates = items.filter { $0.bundle != .reminders }
        let tickets = TicketKeys(knownIn: candidates)
        let keys = candidates.map { Self.keys(of: $0, tickets) }
        let titles = candidates.map { Self.normalized($0.title) }
        var strong: [String: [String]] = [:]
        /// Similar titles, with how close they are, to keep the closest when there are more than `limit`.
        var weak: [String: [(id: String, score: Double, date: Date)]] = [:]

        for i in candidates.indices {
            for j in candidates.indices where j > i && candidates[i].pluginID != candidates[j].pluginID {
                let a = candidates[i].id
                let b = candidates[j].id
                if !keys[i].isDisjoint(with: keys[j]) {
                    strong[a, default: []].append(b)
                    strong[b, default: []].append(a)
                } else if remembered.similar(titles[i], titles[j], by: similarity) {
                    let score = Self.closeness(titles[i], titles[j])
                    weak[a, default: []].append((b, score, candidates[j].date))
                    weak[b, default: []].append((a, score, candidates[i].date))
                }
            }
        }
        remembered.forgetUnused()
        var links: [String: [String]] = [:]
        for id in Set(strong.keys).union(weak.keys) {
            // Closest first, then the most recent: not the order the sources happened to list them in.
            let similar = (weak[id] ?? []).sorted {
                ($0.score, $0.date, $1.id) > ($1.score, $1.date, $0.id)
            }.map(\.id)
            links[id] = Array(((strong[id] ?? []) + similar).prefix(limit))
        }
        return links
    }

    /// How close two titles are, whichever model found them similar: the share of their significant words they have
    /// in common. The same measure on Windows and Linux.
    static func closeness(_ a: String, _ b: String) -> Double {
        let wordsA = KeywordSimilarity.words(a)
        let wordsB = KeywordSimilarity.words(b)
        let union = wordsA.union(wordsB).count
        return union == 0 ? 0 : Double(wordsA.intersection(wordsB).count) / Double(union)
    }

    /// Ticket keys in the title, the context (a Linear identifier lives there) and the preview.
    static func keys(of item: InboxItem, _ tickets: TicketKeys) -> Set<String> {
        tickets.keys([item.title, item.context, item.preview ?? ""].joined(separator: " "))
    }

    /// "[SOAK] chore(deps): Bump axios" → "Bump axios": tags and commit-style prefixes say nothing about the topic.
    static func normalized(_ title: String) -> String {
        title
            .replacingOccurrences(of: #"^(\s*\[[^\]]*\])+\s*"#, with: "", options: .regularExpression)
            .replacingOccurrences(of: #"^[a-zA-Z]+(\([^)]*\))?!?:\s*"#, with: "", options: .regularExpression)
    }
}

/// Whether two titles are similar, as last computed, keyed by the pair of titles whichever order they come in. A pair
/// not looked at during a search is forgotten after it, so the memory follows the inbox.
final class SimilarityMemory: @unchecked Sendable {
    private struct Pair: Hashable {
        let first: String
        let second: String

        init(_ a: String, _ b: String) { (first, second) = a < b ? (a, b) : (b, a) }
    }

    private let lock = NSLock()
    private var answers: [Pair: Bool] = [:]
    private var used: Set<Pair> = []
    /// How many pairs the models were asked about, for tests.
    private(set) var computed = 0

    func similar(_ a: String, _ b: String, by models: [any SimilarityModel]) -> Bool {
        let pair = Pair(a, b)
        let known: Bool? = lock.withLock {
            used.insert(pair)
            return answers[pair]
        }
        if let known { return known }
        // Computed outside the lock: an embedding comparison takes a while, and another search may run meanwhile.
        let answer = models.contains { $0.similar(a, b) }
        lock.withLock {
            answers[pair] = answer
            computed += 1
        }
        return answer
    }

    /// Ends a search: the pairs it didn't look at belong to items gone or renamed.
    func forgetUnused() {
        lock.withLock {
            answers = answers.filter { used.contains($0.key) }
            used = []
        }
    }
}

extension Priority {
    /// The highest of two optional priorities.
    public static func max(_ a: Priority?, _ b: Priority?) -> Priority? {
        switch (a, b) {
        case (let a?, let b?): Swift.max(a, b)
        default: a ?? b
        }
    }
}

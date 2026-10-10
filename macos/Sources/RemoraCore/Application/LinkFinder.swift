import Foundation

/// Finds items from different tools that are about the same thing, even when nobody linked them:
/// a shared ticket key ("CE-1390"), or titles judged similar by the `SimilarityModel`s.
public struct LinkFinder: Sendable {
    public var similarity: [any SimilarityModel]
    /// At most this many links per item, most certain first.
    public var limit: Int

    public init(similarity: [any SimilarityModel] = [KeywordSimilarity()], limit: Int = 3) {
        self.similarity = similarity
        self.limit = limit
    }

    /// Item ID → linked item IDs.
    public func links(_ items: [InboxItem]) -> [String: [String]] {
        let candidates = items.filter { $0.bundle != .reminders }
        let keys = candidates.map { Self.keys(of: $0) }
        let titles = candidates.map { Self.normalized($0.title) }
        var strong: [String: [String]] = [:]
        var weak: [String: [String]] = [:]

        for i in candidates.indices {
            for j in candidates.indices where j > i && candidates[i].pluginID != candidates[j].pluginID {
                let a = candidates[i].id
                let b = candidates[j].id
                if !keys[i].isDisjoint(with: keys[j]) {
                    strong[a, default: []].append(b)
                    strong[b, default: []].append(a)
                } else if similarity.contains(where: { $0.similar(titles[i], titles[j]) }) {
                    weak[a, default: []].append(b)
                    weak[b, default: []].append(a)
                }
            }
        }
        var links: [String: [String]] = [:]
        for id in Set(strong.keys).union(weak.keys) {
            links[id] = Array(((strong[id] ?? []) + (weak[id] ?? [])).prefix(limit))
        }
        return links
    }

    /// Ticket keys in the title, the context (a Linear identifier lives there) and the preview.
    static func keys(of item: InboxItem) -> Set<String> {
        KeywordSimilarity.ticketKeys(
            [item.title, item.context, item.preview ?? ""].joined(separator: " ").uppercased())
    }

    /// "[SOAK] chore(deps): Bump axios" → "Bump axios": tags and commit-style prefixes say nothing about the topic.
    static func normalized(_ title: String) -> String {
        title
            .replacingOccurrences(of: #"^(\s*\[[^\]]*\])+\s*"#, with: "", options: .regularExpression)
            .replacingOccurrences(of: #"^[a-zA-Z]+(\([^)]*\))?!?:\s*"#, with: "", options: .regularExpression)
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

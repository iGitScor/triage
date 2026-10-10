import Foundation

/// Helps with what you're waiting on: who could review, and a friendly nudge when nobody answers.
/// Drafts are written locally from templates; nothing is sent.
public struct WaitingAssistant: Sendable {
    public enum Help: Equatable, Sendable {
        /// Nobody is reviewing yet: people who could.
        case suggestReviewers([Person])
        /// Reviewers asked, no answer for a while.
        case nudge([Person], days: Int)
    }

    /// A reviewer counts as silent after this long without activity.
    public var patience: TimeInterval

    public init(patience: TimeInterval = 86_400) {
        self.patience = patience
    }

    public func help(for item: InboxItem, among items: [InboxItem], me: String?, now: Date = .now) -> Help? {
        guard item.bundle == .awaiting, !item.hasBadge("draft") else { return nil }
        let waitingOn = item.participants.filter { $0.tone == nil }
        if item.participants.isEmpty {
            let people = reviewers(for: item, among: items, me: me)
            return people.isEmpty ? nil : .suggestReviewers(people)
        }
        let silence = now.timeIntervalSince(item.date)
        guard !waitingOn.isEmpty, silence > patience else { return nil }
        return .nudge(waitingOn, days: max(1, Int(silence / 86_400)))
    }

    /// The source's suggestions first, then who reviews most often in the same repo.
    public func reviewers(for item: InboxItem, among items: [InboxItem], me: String?, limit: Int = 3) -> [Person] {
        let excluded = Set([me, item.author?.name].compactMap { $0?.lowercased() })
        let place = item.context.contextKey
        let local =
            items
            .filter { $0.id != item.id && $0.context.contextKey == place }
            .flatMap { $0.participants }
        let frequency = Dictionary(grouping: local, by: \.name).mapValues(\.count)
        let ranked = frequency.sorted { ($0.value, $1.key) > ($1.value, $0.key) }.compactMap { name, _ in
            local.first { $0.name == name }
        }
        var seen = Set<String>()
        return ((item.suggestedPeople ?? []) + ranked)
            .filter { !excluded.contains($0.name.lowercased()) && seen.insert($0.name.lowercased()).inserted }
            .map { Person(name: $0.name, avatarURL: $0.avatarURL) }
            .prefix(limit)
            .map { $0 }
    }

    /// A short, kind message you can paste anywhere.
    public func draft(_ help: Help, for item: InboxItem) -> String {
        let facts = Self.facts(of: item)
        let link = item.url?.absoluteString ?? item.context
        switch help {
        case .suggestReviewers(let people):
            let names = people.prefix(1).map { "@" + $0.name }.joined()
            return L("Hi %@, could you review “%@”? %@\n%@\nThanks!", names, item.title, facts, link)
        case .nudge(let people, let days):
            let names = people.map { "@" + $0.name }.joined(separator: " ")
            let waited = L("%d day", plural: "%d days", days)
            return L(
                "Hi %@, a gentle ping on “%@”: it has been waiting for a review for %@. %@\n%@\nThanks!", names,
                item.title, waited, facts, link)
        }
    }

    /// "It's small (+12 −3) and checks are green." Only what's true and useful.
    static func facts(of item: InboxItem) -> String {
        let small = (item.diffSize ?? .max) < 150
        let green = item.hasBadge("checks.passing")
        switch (small, green) {
        case (true, true):
            return L("It’s small (%@) and checks are green.", item.badges.first { $0.id == "diff" }?.label ?? "")
        case (true, false): return L("It’s small (%@).", item.badges.first { $0.id == "diff" }?.label ?? "")
        case (false, true): return L("Checks are green.")
        case (false, false): return ""
        }
    }
}

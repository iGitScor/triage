import Foundation
import Observation
import RemoraCore

/// The snooze service: every snooze with its reason, and what the advisor learns from them (when to come back, the
/// usual reason, patterns in the pile). The history is saved here on every change. Same on Windows (`snoozes.rs`).
@MainActor @Observable
final class SnoozeService {
    private(set) var history: [SnoozeRecord] { didSet { cachedCounts = nil } }
    /// Patterns in the snoozed pile; recomputed when snoozes change, shown one at a time.
    private(set) var insights: [SnoozeInsight] = []
    private(set) var dismissed: Set<String> = []
    @ObservationIgnored let advisor = SnoozeAdvisor(similarity: [KeywordSimilarity(), EmbeddingSimilarity()])
    @ObservationIgnored private var cachedCounts: (day: Date, counts: [String: Int])?
    @ObservationIgnored private var generation = 0
    /// The latest computation, for tests to wait on.
    @ObservationIgnored private(set) var insightsTask: Task<Void, Never>?
    @ObservationIgnored private let persistence: Persistence

    init(history: [SnoozeRecord], persistence: Persistence) {
        self.history = history
        self.persistence = persistence
    }

    /// A snooze of an item (never a reminder: it isn't a habit). The last 500 are kept.
    func record(_ item: InboxItem, reason: SnoozeReason?, until date: Date) {
        history.append(SnoozeRecord(item: item, reason: reason, at: .now, until: date))
        history = Array(history.suffix(500))
        save()
    }

    /// The snooze that ended in Done: when you finally do things teaches your best hour.
    func done(_ item: InboxItem) {
        guard let index = history.lastIndex(where: { $0.itemID == item.id && $0.doneAt == nil }) else { return }
        history[index].doneAt = .now
        save()
    }

    /// Undo puts the history back as it was.
    func restore(_ history: [SnoozeRecord]) {
        self.history = history
        save()
    }

    /// Erase: nothing is left, and nothing is saved (the files go).
    func reset() {
        history = []
        insights = []
    }

    private func save() {
        persistence.save(history, to: persistence.history)
    }

    // MARK: Advice

    /// Snoozes of each item in the last 30 days, counted once per change of the history and once a day.
    func count(_ item: InboxItem) -> Int {
        let today = Calendar.current.startOfDay(for: .now)
        if let cachedCounts, cachedCounts.day == today { return cachedCounts.counts[item.id] ?? 0 }
        let since = Date.now.addingTimeInterval(-30 * .day)
        let counts = Dictionary(history.filter { $0.at > since }.map { ($0.itemID, 1) }, uniquingKeysWith: +)
        cachedCounts = (today, counts)
        return counts[item.id] ?? 0
    }

    func suggestedReturn(for reason: SnoozeReason) -> Date {
        advisor.suggestedReturn(for: reason, history: history)
    }

    func usualReason(for item: InboxItem) -> SnoozeReason? {
        advisor.usualReason(for: item, history: history)
    }

    func usualReturn(for item: InboxItem) -> Date? {
        advisor.usualReturn(for: item, history: history)
    }

    var visibleInsights: [SnoozeInsight] {
        insights.filter { !dismissed.contains($0.id) }
    }

    /// Patterns in the snoozed pile, worked out off the main thread: clustering compares every pair of titles.
    /// A newer request wins over a slower, older one.
    func refreshInsights(snoozed: [InboxItem], states: [String: ItemState]) {
        let (advisor, history) = (advisor, history)
        generation &+= 1
        let generation = generation
        insightsTask = Task {
            let insights = await Task.detached {
                advisor.insights(snoozed: snoozed, states: states, history: history, now: .now)
            }.value
            guard generation == self.generation else { return }
            self.insights = insights
        }
    }

    func dismiss(_ insight: SnoozeInsight) {
        dismissed.insert(insight.id)
    }
}

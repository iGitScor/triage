import Foundation
import Observation
import RemoraCore

/// The learning service: what Remora learns from your actions on this Mac, and only here. The ranker (what you
/// handle quickly or put off) and how long your timed reviews really took. Both files are saved here on every change.
/// Same on Windows (`learning.rs`).
@MainActor @Observable
final class LearningService {
    /// Silent until it has enough of your actions.
    private(set) var ranker: PersonalRanker { didSet { rankerChanged() } }
    /// Reviews you timed (Start, then Done): how long they really took.
    private(set) var reviewTimings: [ReviewTiming]
    @ObservationIgnored private(set) var records: [ActionRecord]
    /// Inside `batch`: the ranker is worked out once, at the end.
    @ObservationIgnored private var batchDepth = 0
    @ObservationIgnored private var rankerIsStale = false
    @ObservationIgnored private let persistence: Persistence
    /// The ranker orders the inbox: its layout is worked out again.
    @ObservationIgnored var rankerChanged: () -> Void = {}

    init(records: [ActionRecord], reviewTimings: [ReviewTiming], persistence: Persistence) {
        self.records = records
        self.reviewTimings = reviewTimings
        ranker = PersonalRanker(records: records)
        self.persistence = persistence
    }

    /// How your reviews compare with the estimate; 1 until five were timed.
    var reviewPace: Double { ReviewPace.factor(reviewTimings) }

    /// Why an item ranks higher than its priority alone would put it.
    func reason(_ item: InboxItem) -> String? { ranker.reason(item) }

    // MARK: Learning

    /// Opened or finished within a day of its last activity: something you handle quickly.
    func handled(_ item: InboxItem) {
        guard item.bundle != .reminders, Date.now.timeIntervalSince(item.date) < .day else { return }
        learn(ActionRecord(item: item, outcome: .quick))
    }

    /// Snoozed: something you put off. Never a reminder: it isn't a habit.
    func deferred(_ item: InboxItem) {
        learn(ActionRecord(item: item, outcome: .deferred))
    }

    private func learn(_ record: ActionRecord) {
        records = Array((records + [record]).suffix(1_000))
        if batchDepth > 0 {
            rankerIsStale = true
        } else {
            ranker = PersonalRanker(records: records)
        }
        persistence.save(records, to: persistence.learning)
    }

    /// A review started then done: its real duration against the plain estimate, the last 50 kept.
    func reviewed(_ item: InboxItem, startedAt: Date) {
        guard let estimate = ReviewPrep(item)?.estimatedMinutes else { return }
        let actual = Int((Date.now.timeIntervalSince(startedAt) / 60).rounded())
        reviewTimings = Array((reviewTimings + [ReviewTiming(estimated: estimate, actual: actual)]).suffix(50))
        persistence.save(reviewTimings, to: persistence.reviewTimes)
    }

    /// Several actions as one (Mark all as done, Snooze all): the ranker is worked out once, after.
    func batch(_ actions: () -> Void) {
        batchDepth += 1
        actions()
        batchDepth -= 1
        guard batchDepth == 0, rankerIsStale else { return }
        rankerIsStale = false
        ranker = PersonalRanker(records: records)
    }

    /// Undo puts the actions back as they were.
    func restore(_ records: [ActionRecord]) {
        self.records = records
        ranker = PersonalRanker(records: records)
        persistence.save(records, to: persistence.learning)
    }

    /// Erase: nothing is left, and nothing is saved (the files go).
    func reset() {
        records = []
        reviewTimings = []
        ranker = PersonalRanker(records: [])
    }
}

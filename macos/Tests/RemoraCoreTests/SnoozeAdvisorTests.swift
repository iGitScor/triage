import Foundation
import NaturalLanguage
import Testing
@testable import RemoraCore

struct SnoozeAdvisorTests {
    let calendar: Calendar = {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "Europe/Paris")!
        return calendar
    }()
    var advisor: SnoozeAdvisor { SnoozeAdvisor(clock: SnoozeClock(calendar: calendar)) }
    /// Thursday 8 October 2026, 10:00 in Paris.
    var morning: Date { calendar.date(from: DateComponents(year: 2026, month: 10, day: 8, hour: 10))! }

    func record(_ item: InboxItem, reason: SnoozeReason? = nil, at: Date, hours: Double, doneHour: Int? = nil) -> SnoozeRecord {
        var record = SnoozeRecord(item: item, reason: reason, at: at, until: at.addingTimeInterval(hours * 3_600))
        if let doneHour { record.doneAt = calendar.date(bySettingHour: doneHour, minute: 10, second: 0, of: at) }
        return record
    }

    func snoozed(_ items: [InboxItem], until: Date) -> [String: ItemState] {
        Dictionary(uniqueKeysWithValues: items.map { ($0.id, ItemState(snooze: Snooze(until: until, mode: .hide, fingerprint: $0.fingerprint))) })
    }

    // MARK: Reasons

    @Test func reasonsSuggestDifferentReturns() {
        func hour(_ date: Date) -> Int { calendar.component(.hour, from: date) }
        func weekday(_ date: Date) -> Int { calendar.component(.weekday, from: date) }

        let waiting = advisor.suggestedReturn(for: .waiting, history: [], now: morning)
        #expect(calendar.dateComponents([.day], from: morning, to: waiting).day == 2 && hour(waiting) == 9)
        #expect(hour(advisor.suggestedReturn(for: .noTime, history: [], now: morning)) == 13)
        #expect(weekday(advisor.suggestedReturn(for: .notUrgent, history: [], now: morning)) == 2)
        let focus = advisor.suggestedReturn(for: .focus, history: [], now: morning)
        #expect(hour(focus) == 9 && focus > morning)
    }

    @Test func motivationAimsForYourBestHour() {
        let item = makeItem("x")
        let history = (0..<6).map { record(item, at: morning.addingTimeInterval(Double(-$0) * 86_400), hours: 1, doneHour: 15) }
        #expect(advisor.bestHour(history: history) == 15)
        #expect(calendar.component(.hour, from: advisor.suggestedReturn(for: .motivation, history: history, now: morning)) == 15)
        #expect(advisor.bestHour(history: Array(history.prefix(2))) == 9, "not enough data: default 9:00")
    }

    @Test func nudgeDependsOnSize() {
        var big = makeItem("1", bundle: .reviews)
        big.badges = [Badge(id: "diff", label: "+420 −80", tone: .neutral)]
        var small = makeItem("2", bundle: .reviews)
        small.badges = [Badge(id: "diff", label: "+12 −3", tone: .neutral)]
        #expect(advisor.nudge(for: big) == "Do just the first step")
        #expect(advisor.nudge(for: small) == "Start with 10 minutes")
    }

    // MARK: Usual time

    @Test func usualReturnNeedsThreeSnoozesInTheSameContext() {
        let item = makeItem("1")
        let two = (0..<2).map { _ in record(item, at: morning, hours: 2) }
        #expect(advisor.usualReturn(for: item, history: two, now: morning) == nil)
        let three = two + [record(item, at: morning, hours: 2)]
        #expect(advisor.usualReturn(for: item, history: three, now: morning) == morning.addingTimeInterval(2 * 3_600))
        let days = (0..<3).map { _ in record(item, at: morning, hours: 48) }
        let usual = advisor.usualReturn(for: item, history: days, now: morning)!
        #expect(calendar.component(.hour, from: usual) == 9, "long snoozes align to the morning")
    }

    @Test func contextKeyDropsTheItemNumber() {
        #expect("acme/app #12".contextKey == "acme/app")
        #expect("g/p !3".contextKey == "g/p")
        #expect("#releases".contextKey == "#releases")
    }

    // MARK: Insights

    @Test func loopsComeFirst() {
        let item = makeItem("1", date: morning)
        let history = (0..<3).map { record(item, at: morning.addingTimeInterval(Double(-$0) * 86_400), hours: 24) }
        let insights = advisor.insights(snoozed: [item], states: snoozed([item], until: morning + 3_600), history: history, now: morning)
        #expect(insights.first == .loop(item, times: 3))
    }

    @Test func avoidanceNeedsThreeNotFeelingItSnoozes() {
        let item = makeItem("1", date: morning)
        let history = (0..<3).map { record(makeItem("9\($0)"), reason: .motivation, at: morning, hours: 24) }
        let insights = advisor.insights(snoozed: [item], states: snoozed([item], until: morning + 3_600), history: history, now: morning)
        #expect(insights.contains { if case .avoidance(_, 3, [item]) = $0 { true } else { false } })
    }

    @Test func pileUpsAreDetectedAndSpread() {
        let items = (1...4).map { makeItem("\($0)", date: morning) }
        let monday = morning + 4 * 86_400
        let insights = advisor.insights(snoozed: items, states: snoozed(items, until: monday), history: [], now: morning)
        guard case .pileUp(let at, let pile) = insights.first else { Issue.record("no pile-up"); return }
        #expect(pile.count == 4)
        let spread = advisor.spread(pile, from: at)
        #expect(Set(spread.values).count == 4)
        #expect(spread.values.max()! == at + 90 * 60)
    }

    @Test func similarItemsClusterAndStaleOnesShow() {
        var a = makeItem("1", date: morning)
        a.title = "WEB-42 retry uploads"
        var b = makeItem("2", date: morning)
        b.title = "Follow-up on WEB-42"
        var old = makeItem("3", date: morning - 30 * 86_400)
        old.title = "Update the README badges"
        let insights = advisor.insights(snoozed: [a, b, old], states: snoozed([a, b, old], until: morning + 86_400), history: [], now: morning)
        #expect(insights.contains(.cluster([a, b])))
        #expect(insights.contains(.stale([old])))
    }

    @Test func keywordSimilarity() {
        let similarity = KeywordSimilarity()
        #expect(similarity.similar("Retry image uploads", "Image uploads timeout"))
        #expect(!similarity.similar("Retry image uploads", "Lunch tomorrow"))
    }

    /// Calibration pairs plus unseen ones. Precision over recall: related topics with no shared or
    /// near vocabulary ("Speed up the CI pipeline" ~ "Cache dependencies to make builds faster", 1.16)
    /// are missed on purpose rather than risking wrong groups.
    @Test(.enabled(if: NLEmbedding.wordEmbedding(for: .english) != nil))
    func embeddingSimilaritySeparatesTopics() {
        let model = EmbeddingSimilarity()
        #expect(model.similar("Move the image resizer to the new queue", "Move the image queue consumer to SQS"))
        #expect(model.similar("Fix login redirect loop on Safari", "Login fails after SSO redirect"))
        #expect(!model.similar("Q4 hiring scorecard", "Bump eslint to v9"))
        #expect(model.similar("Refactor the search service", "Split search into smaller modules"))
        #expect(!model.similar("Refactor the search service", "Update the privacy policy page"))
        #expect(model.similar("Préparer la démo client", "Slides pour la démo du client"))
        #expect(!model.similar("Speed up the CI pipeline", "Team offsite in November"))
    }

    @Test func untilNewsWakesEvenWhenGloballyOff() {
        let item = makeItem("1")
        let state = ItemState(snooze: Snooze(until: now + 3_600, mode: .hide, fingerprint: "stale", untilNews: true))
        #expect(InboxAssembler(wakeOnActivity: false).placement(of: item, state: state, now: now) == .inbox)
    }

    @Test func statesSavedBeforeReasonsStillDecode() throws {
        let json = #"{"pinned": false, "snooze": {"until": 0, "mode": "hide", "fingerprint": "f"}}"#
        let state = try JSONDecoder().decode(ItemState.self, from: Data(json.utf8))
        #expect(state.snooze?.reason == nil)
    }
}

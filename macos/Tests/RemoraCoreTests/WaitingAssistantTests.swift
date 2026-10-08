import Foundation
import Testing
@testable import RemoraCore

struct WaitingAssistantTests {
    let assistant = WaitingAssistant()

    func mine(_ id: String, participants: [Person] = [], hoursAgo: Double = 1, suggested: [Person]? = nil) -> InboxItem {
        var item = makeItem(id, bundle: .awaiting, date: now.addingTimeInterval(-hoursAgo * 3_600))
        item.context = "acme/app #\(id)"
        item.title = "Add retries"
        item.author = Person(name: "alice")
        item.participants = participants
        item.suggestedPeople = suggested
        item.badges = [Badge(id: "diff", label: "+12 −3", tone: .neutral), Badge(id: "checks.passing", label: "Checks", tone: .positive)]
        item.url = URL(string: "https://github.com/acme/app/pull/\(id)")
        return item
    }

    @Test func suggestsSourceThenLocalReviewersButNeverYou() {
        let others = [mine("2", participants: [Person(name: "frank"), Person(name: "erin")]), mine("3", participants: [Person(name: "erin"), Person(name: "alice")])]
        let item = mine("1", suggested: [Person(name: "dave")])
        #expect(assistant.help(for: item, among: others, me: "alice", now: now) == .suggestReviewers([Person(name: "dave"), Person(name: "erin"), Person(name: "frank")]))
    }

    @Test func nudgesSilentReviewersAfterADay() {
        let waiting = mine("1", participants: [Person(name: "erin"), Person(name: "frank", tone: .accent)], hoursAgo: 50)
        #expect(assistant.help(for: waiting, among: [], me: "alice", now: now) == .nudge([Person(name: "erin")], days: 2))
        let fresh = mine("1", participants: [Person(name: "erin")], hoursAgo: 3)
        #expect(assistant.help(for: fresh, among: [], me: "alice", now: now) == nil)
    }

    @Test func draftsStateOnlyUsefulFacts() {
        let item = mine("1", participants: [Person(name: "erin")], hoursAgo: 50)
        let draft = assistant.draft(.nudge([Person(name: "erin")], days: 2), for: item)
        #expect(draft.contains("@erin") && draft.contains("“Add retries”") && draft.contains("2 day"))
        #expect(draft.contains("small (+12 −3) and checks are green"))
        #expect(draft.contains("https://github.com/acme/app/pull/1"))
    }

    @Test func draftsAndReviewItemsGetNoHelp() {
        var draft = mine("1")
        draft.badges.append(Badge(id: "draft", label: "Draft", tone: .neutral))
        #expect(assistant.help(for: draft, among: [], me: nil, now: now) == nil)
        #expect(assistant.help(for: makeItem("2", bundle: .reviews), among: [], me: nil, now: now) == nil)
    }
}

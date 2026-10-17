import Foundation
import RemoraCore
import Testing

@testable import Remora

/// VoiceOver hears what a row draws besides its text: reviewers and what they did, the review prep, the waiting
/// help (A11Y-19).
@MainActor
struct ItemRowSpeechTests {
    @Test func reviewersPrepAndHelpAreSpoken() throws {
        var item = InboxItem(
            id: "1", accountID: UUID(), pluginID: "github", bundle: .reviews, title: "Retry webhooks",
            context: "acme/app", date: .now)
        item.changes = ChangeSet(files: [
            ChangedFile(path: "src/api/retry.ts", additions: 40, deletions: 0),
            ChangedFile(path: "src/api/retry.test.ts", additions: 30, deletions: 0),
        ])
        let prep = try #require(ReviewPrep(item))
        let reviewers = [Person(name: "Erin", review: .approved), Person(name: "Dave", review: .changesRequested)]
        let spoken = ItemRow.spokenExtras(participants: reviewers, prep: prep, help: nil)
        #expect(spoken.first == "Reviewers: Erin, approved and Dave, changes requested")
        #expect(spoken.contains { $0.hasPrefix("Review: about ") && $0.hasSuffix("minutes, 2 files") })

        let waiting = ItemRow.spokenExtras(
            participants: [], prep: nil, help: .nudge([Person(name: "Bob")], days: 3))
        #expect(waiting == ["Waiting on Bob for 3 days"])
        let nobody = ItemRow.spokenExtras(
            participants: [], prep: nil, help: .suggestReviewers([Person(name: "Ann"), Person(name: "Joe")]))
        #expect(nobody == ["No reviewer yet", "Suggested: Ann and Joe"])
    }
}

import Foundation
import Testing

@testable import RemoraCore

/// When a chat conversation needs nothing more from you: a closing message, or your answer that asked nothing.
struct ConversationTests {
    @Test(arguments: [
        ("Thanks!", TextIntent.closing), ("thank you so much", .closing), ("Merci, parfait", .closing),
        ("Sounds good 👍", .closing), ("👍", .closing), (":+1:", .closing), ("Ok merci !", .closing),
        ("ça marche, à demain", .closing), ("Thanks, can you also send the slides?", .request),
        ("FYI, done", .info), ("Rien à faire de ton côté, merci !", .info),
    ])
    func closingMessages(text: String, expected: TextIntent) {
        #expect(KeywordIntentClassifier().intent(of: text) == expected)
    }

    @Test func aLongMessageWithAThankYouIsNotAClosing() {
        let text = "Thanks for the update, the numbers for the third quarter are in the shared folder with the notes"
        #expect(KeywordIntentClassifier().intent(of: text) != .closing)
    }

    func classified(_ title: String, _ bundle: InboxBundle) -> InboxItem {
        var item = makeItem("c", bundle: bundle, needsAction: true)
        item.title = title
        return VerbClassifier().classify(item)
    }

    @Test func aClosingMessageFinishesTheConversation() {
        let thanks = classified("Thanks, that worked!", .directMessages)
        #expect(thanks.bundle == .finished && !thanks.needsAction)
        #expect(classified("Can you look at this?", .mentions).bundle == .reply)
    }

    /// Your answer, last: a question waits on them; anything else finishes the conversation. A message nobody asked
    /// for (asked) is cleared instead.
    @Test func yourAnswerWaitsOnlyWhenItAsks() {
        let question = classified("Which branch should I use?", .answered)
        #expect(question.bundle == .awaiting && !question.needsAction)
        let done = classified("Here it is", .answered)
        #expect(done.bundle == .finished && done.expires == nil)
        #expect(classified("Here it is", .asked).expires != nil)
    }

    /// A finished conversation is in Done until someone writes again; a pin or a reminder keeps it where it is.
    @Test func aFinishedConversationIsDone() {
        let assembler = InboxAssembler()
        let finished = makeItem("f", bundle: .finished)
        #expect(assembler.placement(of: finished, state: nil, now: now) == .done)
        #expect(assembler.placement(of: finished, state: ItemState(pinned: true), now: now) == .inbox)
        #expect(assembler.layout(items: [finished], states: [:], now: now).done.count == 1)
    }
}

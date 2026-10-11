import Foundation
import Testing

@testable import RemoraCore

/// Reactions (Slack): what they mean, where they put an item, and what the ranker and the assistant learn.
struct ReactionTests {
    func reacted(_ reactions: [Reaction], bundle: InboxBundle = .reply) -> InboxItem {
        var item = makeItem("r", bundle: bundle, needsAction: true)
        item.reactions = reactions
        return item
    }

    let yourCheck = Reaction(name: "white_check_mark", count: 1, mine: true)
    let yourEyes = Reaction(name: "eyes", count: 1, mine: true)

    @Test func meanings() {
        #expect(ReactionMeaning.of("white_check_mark") == .done)
        #expect(Reaction(name: "+1::skin-tone-3", count: 1, mine: false).meaning == .ack, "skin tone dropped")
        #expect(ReactionMeaning.of("eyes") == .started && ReactionMeaning.of("thinking_face") == .question)
        #expect(ReactionMeaning.of("approved-by-design") == .done, "a workspace emoji, by a word of its name")
        #expect(ReactionMeaning.of("merci_beaucoup") == .ack && ReactionMeaning.of("hmm") == .question)
        #expect(ReactionMeaning.of("tada") == .ack && ReactionMeaning.of("pizza") == nil)
    }

    @Test func emojiByName() {
        #expect(Reaction(name: "+1::skin-tone-2", count: 1, mine: false).emoji == "👍")
        #expect(Reaction(name: "approved", count: 1, mine: false).emoji == ":approved:")
    }

    @Test func skinTonesAddUp() {
        let merged = Reaction.merged([
            Reaction(name: "+1", count: 2, mine: false), Reaction(name: "+1::skin-tone-2", count: 1, mine: true),
            Reaction(name: "eyes", count: 1, mine: false),
        ])
        #expect(merged.map(\.name) == ["+1", "eyes"])
        #expect(merged[0].count == 3 && merged[0].mine && merged[0].others == 2)
    }

    /// Your ✅ is Done, your 👀 In progress; a pin or a reminder keeps the item where you put it.
    @Test func yourReactionPlacesTheItem() {
        let assembler = InboxAssembler()
        #expect(assembler.placement(of: reacted([yourCheck]), state: nil, now: now) == .done)
        #expect(assembler.placement(of: reacted([yourEyes, yourCheck]), state: nil, now: now) == .done)
        #expect(assembler.placement(of: reacted([yourEyes]), state: nil, now: now) == .inProgress)
        let theirs = Reaction(name: "white_check_mark", count: 1, mine: false)
        #expect(assembler.placement(of: reacted([theirs]), state: nil, now: now) == .inbox, "only yours")
        #expect(assembler.placement(of: reacted([yourCheck]), state: ItemState(pinned: true), now: now) == .inbox)
        var cleared = ItemState()
        cleared.done = .init(at: now, fingerprint: reacted([yourCheck]).fingerprint, clearedAt: now)
        #expect(assembler.placement(of: reacted([yourCheck]), state: cleared, now: now) == .cleared, "Clear all wins")

        let layout = assembler.layout(items: [reacted([yourCheck])], states: [:], now: now)
        #expect(layout.done.count == 1 && layout.myTurn.isEmpty)
    }

    /// A new 👍 neither brings a Done item back nor wakes a snooze.
    @Test func reactionsAreNotInTheFingerprint() {
        #expect(reacted([]).fingerprint == reacted([Reaction(name: "+1", count: 4, mine: false)]).fingerprint)
    }

    /// Your own message waits on the others only when it asked something.
    @Test func yourQuestionsWait() {
        let classifier = VerbClassifier()
        var question = makeItem("q", bundle: .asked)
        question.title = "Can someone review the mockups?"
        let waiting = classifier.classify(question)
        #expect(waiting.bundle == .awaiting && !waiting.needsAction && waiting.expires == nil)
        #expect(!InboxAssembler().isMyTurn(waiting, state: nil), "in Waiting")

        var update = makeItem("u", bundle: .asked)
        update.title = "Deployed to staging, FYI"
        let cleared = classifier.classify(update)
        #expect(cleared.expires == cleared.date)
        #expect(InboxAssembler().placement(of: cleared, state: nil, now: now) == .cleared, "waits on nobody")
    }

    @Test func theRankerAndTheAssistantSeeWhatReactionsMean() {
        let item = reacted([yourCheck, Reaction(name: "+1", count: 3, mine: true)])
        let features = PersonalRanker.features(of: item)
        #expect(features.contains("reacted:done") && features.contains("reacted:ack"))
        #expect(
            PersonalRanker.features(of: reacted([Reaction(name: "eyes", count: 1, mine: false)])).contains(
                "their:started"))
        #expect(item.reactionNotes == ["you reacted: done", "others reacted: ack ×2"], "meanings and counts, not who")
    }
}

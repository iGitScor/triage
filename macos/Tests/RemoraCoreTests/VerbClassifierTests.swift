import Foundation
import NaturalLanguage
import Testing
@testable import RemoraCore

struct VerbClassifierTests {
    let classifier = VerbClassifier()

    @Test(arguments: [
        ("Can you check the rollout plan?", TextIntent.request),
        ("Tu peux regarder la PR stp", .request),
        ("Merci de valider avant vendredi", .request),
        ("FYI the deploy is done", .info),
        ("@here the office is closed tomorrow", .info),
        ("Pour info, la release est partie", .info),
        ("FYI, can you double check?", .request),
    ])
    func keywordsCatchExplicitWording(text: String, expected: TextIntent) {
        #expect(KeywordIntentClassifier().intent(of: text) == expected)
    }

    @Test func keywordsStayQuietWhenUnsure() {
        #expect(KeywordIntentClassifier().intent(of: "Lunch tomorrow") == nil)
    }

    @Test func authoredMergeRequestsGetAVerb() {
        func authored(_ badges: [String]) -> InboxBundle {
            classifier.classify(makeItem("1", bundle: .authored, badges: badges.map { Badge(id: $0, label: $0, tone: .neutral) })).bundle
        }
        #expect(authored(["approved"]) == .merge)
        #expect(authored(["approved", "checks.failing"]) == .fix)
        #expect(authored(["changes"]) == .fix)
        #expect(authored(["draft", "changes"]) == .awaiting)
        #expect(authored([]) == .awaiting)
    }

    @Test func chatBecomesReplyOrRead() {
        var question = makeItem("1", bundle: .mentions)
        question.title = "Can you review the migration?"
        var news = makeItem("2", bundle: .mentions)
        news.title = "FYI the migration is done"

        let reply = classifier.classify(question)
        #expect(reply.bundle == .reply && reply.needsAction)
        let read = classifier.classify(news)
        #expect(read.bundle == .read && !read.needsAction)
    }

    @Test func unsureChatDefaultsToReply() {
        var dm = makeItem("1", bundle: .directMessages)
        dm.title = "Lunch tomorrow"
        #expect(VerbClassifier([]).classify(dm).bundle == .reply)
    }

    @Test func readItemsAreShownButNotCounted() {
        let items = [makeItem("1", bundle: .read), makeItem("2", bundle: .reply, needsAction: true)]
        let layout = InboxAssembler().layout(items: items, states: [:], now: now)
        #expect(layout.myTurnItems.count == 2)
        #expect(layout.actionCount == 1)
    }

    /// Sentences the model never saw as examples, in both languages. macOS downloads the embeddings on
    /// demand, so a fresh machine (CI) may not have the French one yet.
    @Test(.enabled(if: NLEmbedding.sentenceEmbedding(for: .english) != nil && NLEmbedding.sentenceEmbedding(for: .french) != nil))
    func embeddingsSortUnseenSentences() {
        let model = EmbeddingIntentClassifier()
        let cases: [(String, TextIntent)] = [
            ("Would love your feedback on the new onboarding flow", .request),
            ("Are you free to pair on the bug this afternoon", .request),
            ("Need your sign-off on the budget", .request),
            ("The incident is resolved and the postmortem is published", .info),
            ("New dashboard is live for the whole team", .info),
            ("Ton retour sur la nouvelle maquette serait précieux", .request),
            ("Le correctif est en production depuis ce matin", .info),
            ("Tu pourrais jeter un œil au ticket", .request),
            ("La démo s'est très bien passée", .info),
            ("Il faudrait que tu valides le budget", .request),
            ("Les accès sont rétablis pour tout le monde", .info),
        ]
        let results = cases.map { (text, expected) in (text, expected, model.intent(of: text)) }
        for (text, expected, got) in results {
            print("[embedding]", got.map { "\($0)" } ?? "unsure", "expected:", expected, "—", text)
        }
        let correct = results.filter { $0.1 == $0.2 }.count
        let wrong = results.filter { $0.2 != nil && $0.1 != $0.2 }.count
        #expect(wrong == 0, "confident mistakes: \(wrong)")
        #expect(correct >= 8, "correct: \(correct) of \(cases.count)")
    }
}

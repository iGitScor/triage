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
        // A curly apostrophe, the formal form and no hyphen, as people type at work. Same cases as Windows.
        ("S’il vous plaît, relisez la spec avant demain", .request),
        ("Qu’en pensez-vous", .request),
        ("Qu'en pensez vous, Marc", .request),
        ("Peux tu regarder la PR", .request),
        ("Pourriez vous valider le devis", .request),
        ("À titre d’info, le build passe", .info),
    ])
    func keywordsCatchExplicitWording(text: String, expected: TextIntent) {
        #expect(KeywordIntentClassifier().intent(of: text) == expected)
    }

    /// A negated request asks nothing; a real request elsewhere in the message still does.
    @Test(
        arguments: [
            ("No need to reply, the build is green.", TextIntent.info),
            ("Pas besoin de répondre, c’est réglé.", .info),
            ("FYI no action needed on your side", .info),
            ("Rien à faire de ton côté, merci !", .info),
            ("Not urgent, just sharing the doc", nil),
            ("Pas urgent, juste pour te tenir au courant", nil),
            ("No need to reply, but can you check the doc", .request),
            ("Not urgent, but could you look at it", .request),
            ("Urgent: the deploy is broken", .request),
        ] as [(String, TextIntent?)])
    func negatedRequestsAskNothing(text: String, expected: TextIntent?) {
        #expect(KeywordIntentClassifier().intent(of: text) == expected)
    }

    /// "Merci de" before something to do asks; before what was given or done it thanks. Same cases as Windows.
    @Test(
        arguments: [
            ("Invitation envoyée pour la rétro, merci d'accepter", TextIntent.request),
            ("Merci de relire avant midi", .request),
            ("Merci de me renvoyer le fichier", .request),
            ("Merci de bien vouloir signer le contrat", .request),
            ("Merci de ne pas merger avant demain", .request),
            ("Merci de la relire", .request),
            ("Merci de ton retour", .closing),
            ("Merci de votre aide !", .closing),
            ("Merci d'avoir regardé si vite", .closing),
            ("Merci d’avance", .closing),
        ] as [(String, TextIntent?)])
    func merciDeAsksOnlyBeforeSomethingToDo(text: String, expected: TextIntent?) {
        #expect(KeywordIntentClassifier().intent(of: text) == expected)
    }

    @Test func keywordsStayQuietWhenUnsure() {
        #expect(KeywordIntentClassifier().intent(of: "Lunch tomorrow") == nil)
    }

    /// Nothing but code or a link is to read, not to reply. Same cases as Windows.
    @Test(
        arguments: [
            ("```\nlet total = items.count\n```", TextIntent.info),
            ("https://example.com/runbook", .info),
            ("`make check`", .info),
            ("   ", nil),
            ("https://example.com/runbook can you check?", .request),
        ] as [(String, TextIntent?)])
    func codeAndLinksAloneAreToRead(text: String, expected: TextIntent?) {
        #expect(KeywordIntentClassifier().intent(of: text) == expected)
    }

    /// A message clearly in another language isn't read with the English examples; short words still are.
    @Test func otherLanguagesStayUnsure() {
        #expect(EmbeddingIntentClassifier.language(of: "Kannst du dir das bitte ansehen?") == nil)
        #expect(EmbeddingIntentClassifier.language(of: "¿Puedes revisar esto hoy?") == nil)
        #expect(EmbeddingIntentClassifier.language(of: "ok") == .english)
        #expect(EmbeddingIntentClassifier.language(of: "Deploy done") == .english)
        #expect(EmbeddingIntentClassifier.language(of: "Tu peux regarder ?") == .french)
        #expect(EmbeddingIntentClassifier().intent(of: "Kannst du dir das bitte ansehen?") == nil)
    }

    /// Whole words, a question mark that ends a sentence, links and code left out. Same cases as Windows.
    @Test(
        arguments: [
            ("Pleased to share the new office plan", nil),
            ("See https://example.com/search?q=remora for the numbers", nil),
            ("Use `a ?? b` when the value can be missing", nil),
            ("The stpierre account is migrated", nil),
            ("Thoughtful review, merged", nil),
            ("Is the deploy done?", TextIntent.request),
            ("Ready? Let's ship it", .request),
            ("Tu peux regarder ?", .request),
            ("Can you check (the second link)?", .request),
            ("please merge", .request),
            ("Heads-up: the API moves on Monday", .info),
            ("FYI: www.example.com/faq?x=1 is updated", .info),
        ] as [(String, TextIntent?)])
    func keywordsMatchWordsNotFragments(text: String, expected: TextIntent?) {
        #expect(KeywordIntentClassifier().intent(of: text) == expected)
    }

    @Test func authoredMergeRequestsGetAVerb() {
        func authored(_ badges: [String]) -> InboxBundle {
            classifier.classify(
                makeItem(
                    "1", bundle: .authored,
                    badges: badges.map { Badge(id: BadgeID(rawValue: $0), label: $0, tone: .neutral) })
            ).bundle
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
    @Test(
        .enabled(
            if: NLEmbedding.sentenceEmbedding(for: .english) != nil
                && NLEmbedding.sentenceEmbedding(for: .french) != nil))
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
        // Information needs a wider gap than a request (`leavingMargin`), so some stays unsure: the labelled set of
        // eval/ (IntentEvaluationTests) measures the balance.
        #expect(correct >= 6, "correct: \(correct) of \(cases.count)")
    }

    /// Messages classified at the same time, while the models load, get the answers they get one at a time.
    @Test(
        .enabled(
            if: NLEmbedding.sentenceEmbedding(for: .english) != nil
                && NLEmbedding.sentenceEmbedding(for: .french) != nil))
    func concurrentMessagesGetTheSameAnswers() {
        let texts = [
            "Would love your feedback on the new onboarding flow", "The incident is resolved",
            "Ton retour sur la maquette serait précieux", "La démo s'est très bien passée",
            "Need your sign-off on the budget", "Les accès sont rétablis pour tout le monde",
        ]
        let expected = texts.map { EmbeddingIntentClassifier().intent(of: $0) }
        let shared = EmbeddingIntentClassifier()
        final class Answers: @unchecked Sendable {
            var values = [TextIntent??](repeating: nil, count: 60)
            let lock = NSLock()
        }
        let answers = Answers()
        DispatchQueue.concurrentPerform(iterations: 60) { i in
            let answer = shared.intent(of: texts[i % texts.count])
            answers.lock.lock()
            answers.values[i] = .some(answer)
            answers.lock.unlock()
        }
        for (i, answer) in answers.values.enumerated() {
            #expect(answer == .some(expected[i % texts.count]), "\(texts[i % texts.count])")
        }
    }

    /// The examples are embedded once; the distance computed from their vectors must be Apple's, so the
    /// calibrated margin keeps its meaning.
    @Test(.enabled(if: NLEmbedding.sentenceEmbedding(for: .english) != nil))
    func cachedVectorsGiveApplesDistance() throws {
        let embedding = try #require(NLEmbedding.sentenceEmbedding(for: .english))
        let pairs = [
            ("Can you review my pull request today", "Could you take a look at this"),
            ("The deploy is done", "Would love your feedback on the new onboarding flow"),
        ]
        for (a, b) in pairs {
            let ours = EmbeddingIntentClassifier.cosineDistance(
                try #require(embedding.vector(for: a)), try #require(embedding.vector(for: b)))
            #expect(abs(ours - embedding.distance(between: a, and: b, distanceType: .cosine)) < 1e-6, "\(a) / \(b)")
        }
    }

    /// Each sort says why: the words that decided, a question, or nothing clear. Same cases as Windows.
    @Test(
        arguments: [
            ("Can you check the build?", SortReason(.keyword, "can you")),
            ("Tu peux regarder ?", SortReason(.keyword, "tu peux")),
            ("Merci de relire la PR", SortReason(.keyword, "merci de")),
            ("Is the build green?", SortReason(.question)),
            ("No need to reply, the build is green", SortReason(.noReply, "no need to reply")),
            ("FYI the build is green", SortReason(.info, "fyi")),
            ("Thanks!", SortReason(.closing, "thanks")),
            ("👍", SortReason(.closing)),
            ("https://example.com/build/42", SortReason(.codeOnly)),
        ])
    func eachSortSaysWhy(text: String, reason: SortReason) {
        #expect(KeywordIntentClassifier().classified(text)?.reason == reason)
    }

    @Test func aGuessSaysSoAndTheSourcesSortHasNoReason() {
        var dm = makeItem("1", bundle: .directMessages)
        dm.title = "The client is still waiting for the quote"
        let unsure = classifier.classify(dm)
        #expect(unsure.bundle == .reply)
        #expect(unsure.reason == SortReason(.unsure), "kept in To reply, and says it's a guess")
        var review = makeItem("2", bundle: .reviews)
        review.title = "Can you review?"
        #expect(classifier.classify(review).reason == nil, "the source sorted it, its badges tell why")
    }

    /// Put in To reply by the user: there whatever its words say, until it's done; never the other way.
    @Test func aMessagePutInToReplyStaysThere() {
        func classified(_ id: String, _ title: String, _ bundle: InboxBundle) -> InboxItem {
            var item = makeItem(id, bundle: bundle, needsAction: true)
            item.title = title
            return classifier.classify(item)
        }
        let news = classified("n", "FYI the build is green", .mentions)
        let thanks = classified("t", "Thanks!", .directMessages)
        let mine = classified("m", "Here is the link", .asked)
        #expect(news.canMoveToReply && thanks.canMoveToReply && !mine.canMoveToReply)
        let moved = ItemState(movedToReplyAt: now)
        let layout = InboxAssembler().layout(items: [news, thanks], states: ["n": moved, "t": moved], now: now)
        #expect(layout.myTurnItems.count == 2, "both in To reply, the finished one too")
        #expect(
            layout.myTurnItems.allSatisfy { $0.bundle == .reply && $0.needsAction && $0.reason == SortReason(.moved) })
    }

    /// A long message is read by its first two sentences and its last one; a short one whole.
    @Test func aLongMessageIsReadByItsGist() {
        let long = "The build broke last night. The cache was stale. We purged it. It took an hour. Can you check?"
        #expect(
            EmbeddingIntentClassifier.gist(long) == "The build broke last night. The cache was stale. Can you check?")
        #expect(EmbeddingIntentClassifier.gist("Short one. Two. Three.") == "Short one. Two. Three.")
    }

    /// The answers kept stay capped, the oldest out first.
    @Test func theAnswerCacheIsCapped() {
        let classifier = EmbeddingIntentClassifier()
        for n in 0..<(EmbeddingIntentClassifier.kept + 50) { _ = classifier.intent(of: "message \(n)") }
        #expect(classifier.cachedCount == EmbeddingIntentClassifier.kept)
    }
}

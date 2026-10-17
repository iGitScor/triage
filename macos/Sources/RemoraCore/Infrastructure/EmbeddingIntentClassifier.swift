import Foundation
import NaturalLanguage

/// Guesses intent by comparing a message with example sentences, using Apple's on-device sentence
/// embeddings (English and French). No network, no generative model.
public final class EmbeddingIntentClassifier: TextIntentClassifier, @unchecked Sendable {
    static let examples: [NLLanguage: [TextIntent: [String]]] = [
        .english: [
            .request: [
                "Can you take a look at this?", "Could you review my changes today?",
                "What do you think about this approach?", "Do you have a minute to talk?",
                "I need your approval before I deploy.", "When can you send me the numbers?",
                "I'd love your opinion on this document.", "Your feedback would really help.",
                "I need your answer before noon.", "Can you approve the request?",
                "Are you available for a quick call tomorrow?", "Waiting for your go-ahead.",
            ],
            .info: [
                "The deployment is done.", "Here are the notes from today's meeting.",
                "The release went out this morning.", "The new pricing starts next month.",
                "Sharing the slides from the presentation.", "The office will be closed on Friday.",
                "The migration went smoothly.", "The bug is fixed in production.",
                "A new version is available for the whole team.", "The incident is resolved.",
                "The dashboard is now live.", "Good news, the client signed.",
            ],
            .closing: [
                "Thanks, that worked.", "Thank you so much!", "Perfect, thanks.", "Sounds good to me.",
                "Got it, thanks!", "Great, see you tomorrow.", "Ok, noted.", "Awesome, appreciated.",
                "No problem at all.", "All good, thanks for the help.", "Cool, will do.", "Makes sense, thanks.",
            ],
        ],
        .french: [
            .request: [
                "Tu peux regarder ça ?", "Pourrais-tu relire mes modifications aujourd'hui ?",
                "Qu'est-ce que tu en penses ?", "Tu as une minute pour en parler ?",
                "J'ai besoin de ta validation avant de déployer.", "Quand peux-tu m'envoyer les chiffres ?",
                "J'aimerais avoir ton avis sur ce document.", "Ton retour m'aiderait beaucoup.",
                "Il me faut ta réponse avant midi.", "Peux-tu valider la demande ?",
                "Tu serais dispo pour un point demain ?", "J'attends ton feu vert.",
            ],
            .info: [
                "Le déploiement est terminé.", "Voici le compte rendu de la réunion.",
                "La version est sortie ce matin.", "Les nouveaux tarifs commencent le mois prochain.",
                "Je partage les slides de la présentation.", "Le bureau sera fermé vendredi.",
                "La migration s'est bien passée.", "Le bug est corrigé en production.",
                "Nouvelle version disponible pour toute l'équipe.", "L'incident est résolu.",
                "Le tableau de bord est en ligne.", "Bonne nouvelle, le client a signé.",
            ],
            .closing: [
                "Merci, ça a marché.", "Merci beaucoup !", "Parfait, merci.", "Ça me va.",
                "C'est noté, merci !", "Super, à demain.", "Ok, bien reçu.", "Top, merci pour l'aide.",
                "Pas de souci.", "Nickel, merci.", "Ça marche, je m'en occupe.", "D'accord, merci.",
            ],
        ],
    ]

    /// How many nearest examples are averaged per class.
    private let neighbors = 3
    /// Minimum gap between the two classes' distances (Apple's cosine scale) to trust the answer.
    private let margin: Double
    /// The gap needed to answer information or closing, which takes a message out of *To reply*: a request missed
    /// costs more than one shown for nothing.
    private let leavingMargin: Double

    /// A language's sentence embedding and the examples' vectors in it, computed once: comparing with
    /// `distance(between:and:)` embedded every example again for every message.
    private final class Model: @unchecked Sendable {
        let embedding: NLEmbedding
        let examples: [TextIntent: [[Double]]]
        /// `NLEmbedding` isn't documented as thread-safe: one message at a time per language, without holding up
        /// the cache or the other language.
        let lock = NSLock()

        init(embedding: NLEmbedding, examples: [TextIntent: [[Double]]]) {
            self.embedding = embedding
            self.examples = examples
        }

        func vector(for text: String) -> [Double]? {
            lock.lock()
            defer { lock.unlock() }
            return embedding.vector(for: text)
        }
    }

    /// Guards `models` and `answers`, held only to read or write them: a model loads, and a message is
    /// classified, outside it.
    private let lock = NSLock()
    /// Loading a model (possibly downloaded the first time) happens once per language, here.
    private let loading = NSLock()
    /// Nil when the language has no model on this Mac.
    private var models: [NLLanguage: Model?] = [:]
    /// The same messages come back on every refresh: answer each one once. Capped, oldest out first, so a Mac left
    /// running for weeks doesn't keep every message it ever saw.
    private var answers: [String: TextIntent?] = [:]
    /// `answers`' keys, oldest first.
    private var answered: [String] = []
    /// More than several refreshes' worth of messages.
    static let kept = 1_000

    public init(margin: Double = 0.05, leavingMargin: Double = 0.07) {
        self.margin = margin
        self.leavingMargin = leavingMargin
    }

    /// What of a message is read: its first two sentences and its last one. A long message, the whole of it in one
    /// vector, means little; what's asked is said first ("Can you…") or last ("…let me know what you think").
    static func gist(_ text: String) -> String {
        let tokenizer = NLTokenizer(unit: .sentence)
        tokenizer.string = text
        let sentences = tokenizer.tokens(for: text.startIndex..<text.endIndex).map {
            text[$0].trimmingCharacters(in: .whitespacesAndNewlines)
        }.filter { !$0.isEmpty }
        guard sentences.count > 3 else { return text }
        return (sentences.prefix(2) + [sentences[sentences.count - 1]]).joined(separator: " ")
    }

    /// How many answers are kept now (tests).
    var cachedCount: Int {
        lock.lock()
        defer { lock.unlock() }
        return answers.count
    }

    /// Loads both languages' models, so the first refresh doesn't wait for them. Call it off the main thread.
    public func warmUp() {
        for language in Self.examples.keys { _ = model(for: language) }
    }

    public func intent(of text: String) -> TextIntent? {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return nil }
        if let answer = cached(text) { return answer }
        let answer = classify(text)
        lock.lock()
        if answers.updateValue(answer, forKey: text) == nil { answered.append(text) }
        if answered.count > Self.kept {
            let gone = answered.count - Self.kept
            for old in answered.prefix(gone) { answers[old] = nil }
            answered.removeFirst(gone)
        }
        lock.unlock()
        return answer
    }

    private func cached(_ text: String) -> TextIntent?? {
        lock.lock()
        defer { lock.unlock() }
        return answers[text]
    }

    private func classify(_ text: String) -> TextIntent? {
        // Code and links say nothing of what's asked, and would pull the sentence's meaning their way; a long message
        // is read by its opening and its last sentence, where what's asked is.
        let text = Self.gist(KeywordIntentClassifier.prose(text).trimmingCharacters(in: .whitespacesAndNewlines))
        guard let language = Self.language(of: text), let model = model(for: language),
            let message = model.vector(for: text)
        else { return nil }
        let examples = model.examples

        func distance(to intent: TextIntent) -> Double {
            let nearest = (examples[intent] ?? []).map { Self.cosineDistance(message, $0) }.sorted().prefix(neighbors)
            return nearest.isEmpty ? .infinity : nearest.reduce(0, +) / Double(nearest.count)
        }
        // The nearest class, if clearly nearer than the runner-up.
        let ranked = examples.keys.map { ($0, distance(to: $0)) }.filter { $0.1.isFinite }.sorted { $0.1 < $1.1 }
        guard ranked.count >= 2,
            ranked[1].1 - ranked[0].1 >= (ranked[0].0 == .request ? margin : max(margin, leavingMargin))
        else { return nil }
        return ranked[0].0
    }

    private func model(for language: NLLanguage) -> Model? {
        lock.lock()
        let known = models[language]
        lock.unlock()
        if let known { return known }
        loading.lock()
        defer { loading.unlock() }
        lock.lock()
        let loadedMeanwhile = models[language]
        lock.unlock()
        if let loadedMeanwhile { return loadedMeanwhile }
        let model = Self.load(language)
        lock.lock()
        models[language] = .some(model)
        lock.unlock()
        return model
    }

    private static func load(_ language: NLLanguage) -> Model? {
        guard let embedding = NLEmbedding.sentenceEmbedding(for: language), let examples = examples[language] else {
            return nil
        }
        let vectors = examples.mapValues { sentences in sentences.compactMap { embedding.vector(for: $0) } }
        return Model(embedding: embedding, examples: vectors)
    }

    /// The distance `NLEmbedding.distance(between:and:)` gives with its cosine type, measured: √(2 · (1 − cos(a, b))),
    /// the distance between the two vectors scaled to length 1, from 0 to 2. Same scale, so `margin` keeps its meaning.
    static func cosineDistance(_ a: [Double], _ b: [Double]) -> Double {
        guard a.count == b.count, !a.isEmpty else { return .infinity }
        var dot = 0.0
        var normA = 0.0
        var normB = 0.0
        for i in a.indices {
            dot += a[i] * b[i]
            normA += a[i] * a[i]
            normB += b[i] * b[i]
        }
        guard normA > 0, normB > 0 else { return .infinity }
        let cosine = dot / (normA.squareRoot() * normB.squareRoot())
        return (2 * max(0, 1 - cosine)).squareRoot()
    }

    /// English or French, or nil for a message clearly in another language: the English examples would only
    /// guess at German or Spanish, so the message stays unsure. Short words ("ok", "PR") read as English.
    static func language(of text: String) -> NLLanguage? {
        let recognizer = NLLanguageRecognizer()
        recognizer.processString(text)
        if let (other, confidence) = recognizer.languageHypotheses(withMaximum: 1).first,
            !examples.keys.contains(other), confidence >= otherLanguageConfidence
        {
            return nil
        }
        recognizer.reset()
        recognizer.languageConstraints = Array(examples.keys)
        recognizer.processString(text)
        return recognizer.dominantLanguage ?? .english
    }

    /// How sure the recognizer must be of another language to leave a message unsure: "Hast du kurz Zeit?" is
    /// German at 1.0, "ok" Polish at 0.3.
    static let otherLanguageConfidence = 0.8
}

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
                "The release went out this morning.", "Thanks, that worked.",
                "Sharing the slides from the presentation.", "The office will be closed on Friday.",
                "The migration went smoothly.", "The bug is fixed in production.",
                "A new version is available for the whole team.", "The incident is resolved.",
                "The dashboard is now live.", "Good news, the client signed.",
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
                "La version est sortie ce matin.", "Merci, ça a marché.",
                "Je partage les slides de la présentation.", "Le bureau sera fermé vendredi.",
                "La migration s'est bien passée.", "Le bug est corrigé en production.",
                "Nouvelle version disponible pour toute l'équipe.", "L'incident est résolu.",
                "Le tableau de bord est en ligne.", "Bonne nouvelle, le client a signé.",
            ],
        ],
    ]

    /// How many nearest examples are averaged per class.
    private let neighbors = 3
    /// Minimum gap between the two classes' distances (Apple's cosine scale) to trust the answer.
    private let margin: Double
    private let lock = NSLock()
    private var embeddings: [NLLanguage: NLEmbedding] = [:]
    /// The same messages come back on every refresh: answer each one once.
    private var answers: [String: TextIntent?] = [:]

    public init(margin: Double = 0.05) {
        self.margin = margin
    }

    public func intent(of text: String) -> TextIntent? {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return nil }
        lock.lock()
        defer { lock.unlock() }
        if let answer = answers[text] { return answer }
        let answer = classify(text)
        answers[text] = answer
        return answer
    }

    private func classify(_ text: String) -> TextIntent? {
        let language = Self.language(of: text)
        guard let embedding = embedding(for: language), let examples = Self.examples[language] else { return nil }

        func distance(to intent: TextIntent) -> Double {
            let nearest = (examples[intent] ?? []).map { embedding.distance(between: text, and: $0) }.sorted().prefix(neighbors)
            return nearest.isEmpty ? .infinity : nearest.reduce(0, +) / Double(nearest.count)
        }
        let request = distance(to: .request)
        let info = distance(to: .info)
        guard request.isFinite, info.isFinite, abs(request - info) >= margin else { return nil }
        return request < info ? .request : .info
    }

    private func embedding(for language: NLLanguage) -> NLEmbedding? {
        if let cached = embeddings[language] { return cached }
        let loaded = NLEmbedding.sentenceEmbedding(for: language)
        embeddings[language] = loaded
        return loaded
    }

    static func language(of text: String) -> NLLanguage {
        let recognizer = NLLanguageRecognizer()
        recognizer.languageConstraints = [.english, .french]
        recognizer.processString(text)
        return recognizer.dominantLanguage ?? .english
    }
}

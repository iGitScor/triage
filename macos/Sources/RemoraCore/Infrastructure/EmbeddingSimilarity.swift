import Foundation
import NaturalLanguage

/// Tells whether two titles share a topic using Apple's on-device word embeddings (English and French):
/// each significant word is matched with its closest word in the other title, and the distances averaged.
/// Sentence embeddings were tried first and did not separate short technical titles.
public final class EmbeddingSimilarity: SimilarityModel, @unchecked Sendable {
    /// Calibrated on title pairs: same topic ≤ 0.89; unrelated pairs seen as low as 1.046 on real data.
    private let threshold: Double
    private let lock = NSLock()
    private var embeddings: [NLLanguage: NLEmbedding] = [:]

    public init(threshold: Double = 0.95) {
        self.threshold = threshold
    }

    public func similar(_ a: String, _ b: String) -> Bool {
        distance(a, b).map { $0 <= threshold } ?? false
    }

    public func distance(_ a: String, _ b: String) -> Double? {
        let wordsA = Self.words(a)
        let wordsB = Self.words(b)
        guard !wordsA.isEmpty, !wordsB.isEmpty,
            let embedding = embedding(for: EmbeddingIntentClassifier.language(of: a + " " + b))
        else { return nil }

        func side(_ from: [String], _ to: [String]) -> Double {
            let scores = from.map { word in
                to.map { other in
                    word == other
                        ? 0
                        : (embedding.contains(word) && embedding.contains(other)
                            ? embedding.distance(between: word, and: other) : 2)
                }.min() ?? 2
            }
            return scores.reduce(0, +) / Double(scores.count)
        }
        return (side(wordsA, wordsB) + side(wordsB, wordsA)) / 2
    }

    static func words(_ text: String) -> [String] {
        text.lowercased()
            .split { !$0.isLetter }
            .map(String.init)
            .filter { $0.count >= 3 && !KeywordSimilarity.stopWords.contains($0) }
    }

    private func embedding(for language: NLLanguage) -> NLEmbedding? {
        lock.lock()
        defer { lock.unlock() }
        if let cached = embeddings[language] { return cached }
        let loaded = NLEmbedding.wordEmbedding(for: language)
        embeddings[language] = loaded
        return loaded
    }
}

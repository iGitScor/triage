import Foundation
import NaturalLanguage

/// Tells whether two titles share a topic using Apple's on-device word embeddings (English and French):
/// each significant word is matched with its closest word in the other title, and the distances averaged.
/// Sentence embeddings were tried first and did not separate short technical titles.
public final class EmbeddingSimilarity: SimilarityModel, @unchecked Sendable {
    /// Grouping snoozed items by topic. Calibrated on title pairs: same topic ≤ 0.89; unrelated pairs seen as low as
    /// 1.046 on real data.
    public static let topicThreshold = 0.95
    /// Linking items across tools: stricter, since a wrong link is shown on the item itself.
    public static let linkThreshold = 0.9

    private let threshold: Double
    private let lock = NSLock()
    private var embeddings: [NLLanguage: NLEmbedding] = [:]
    /// Word-to-word distances already measured (ML-10): the same words come back across titles and refreshes.
    /// Emptied past `wordPairLimit`, so it can't grow without bound.
    private var wordDistances: [String: Double] = [:]
    static let wordPairLimit = 50_000

    public init(threshold: Double = EmbeddingSimilarity.topicThreshold) {
        self.threshold = threshold
    }

    public func similar(_ a: String, _ b: String) -> Bool {
        distance(a, b).map { $0 <= threshold } ?? false
    }

    public func distance(_ a: String, _ b: String) -> Double? {
        let wordsA = Self.words(a)
        let wordsB = Self.words(b)
        guard !wordsA.isEmpty, !wordsB.isEmpty,
            let embedding = embedding(for: EmbeddingIntentClassifier.language(of: a + " " + b) ?? .english)
        else { return nil }

        func side(_ from: [String], _ to: [String]) -> Double {
            let scores = from.map { word in
                to.map { other in word == other ? 0 : wordDistance(word, other, in: embedding) }.min() ?? 2
            }
            return scores.reduce(0, +) / Double(scores.count)
        }
        return (side(wordsA, wordsB) + side(wordsB, wordsA)) / 2
    }

    private func wordDistance(_ a: String, _ b: String, in embedding: NLEmbedding) -> Double {
        let key = "\(embedding.language?.rawValue ?? "")|" + (a < b ? "\(a)|\(b)" : "\(b)|\(a)")
        if let known = lock.withLock({ wordDistances[key] }) { return known }
        let distance =
            embedding.contains(a) && embedding.contains(b) ? embedding.distance(between: a, and: b) : 2
        lock.withLock {
            if wordDistances.count >= Self.wordPairLimit { wordDistances = [:] }
            wordDistances[key] = distance
        }
        return distance
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

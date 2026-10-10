import Foundation
import NaturalLanguage

/// Two titles sharing a distinctive technical word (a library, product or code name such as "axios",
/// "nanoid", "dd-trace", "Dotfile") are about the same thing. A word counts as distinctive when it holds a
/// digit or a dash, or when it isn't in the on-device English or French vocabulary.
public final class RareTokenSimilarity: SimilarityModel, @unchecked Sendable {
    private let vocabularies: [NLEmbedding]

    public init() {
        vocabularies = [NLLanguage.english, .french].compactMap { NLEmbedding.wordEmbedding(for: $0) }
    }

    public func similar(_ a: String, _ b: String) -> Bool {
        !rareTokens(a).isDisjoint(with: rareTokens(b))
    }

    func rareTokens(_ text: String) -> Set<String> {
        let tokens = text.lowercased()
            .split { !($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_" || $0 == ".") }
            .map { String($0).trimmingCharacters(in: CharacterSet(charactersIn: "-_.")) }
            .filter { $0.count >= 4 && $0.contains(where: \.isLetter) && !KeywordSimilarity.stopWords.contains($0) }
        return Set(
            tokens.filter { token in
                token.contains { $0.isNumber || $0 == "-" || $0 == "_" }
                    || !vocabularies.contains { $0.contains(token) }
            })
    }
}

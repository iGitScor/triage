import Foundation
import NaturalLanguage

/// Two titles sharing a distinctive technical word (a library, product or code name such as "axios",
/// "nanoid", "dd-trace", "Dotfile") are about the same thing. A word counts as distinctive when it holds a
/// digit or a dash, or when it isn't in the on-device English or French vocabulary, and is neither a typo of a word
/// that is (one letter changed, added, dropped, or two swapped: "recieve") nor a person's name ("Sacha"): those are
/// shared by unrelated titles all the time.
///
/// macOS downloads those vocabularies on demand. While one is missing, every word of its language would look
/// distinctive and an ordinary word ("réunion", "address") would link unrelated items: then only words with a digit,
/// a dash or an underscore count.
public final class RareTokenSimilarity: SimilarityModel, @unchecked Sendable {
    private let vocabularies: [(String) -> Bool]
    /// Both vocabularies loaded: ordinary words can be told apart.
    private let complete: Bool
    /// Whether an out-of-vocabulary word is a typo, measured once per word.
    private var typos: [String: Bool] = [:]
    private let lock = NSLock()

    public convenience init() {
        let embeddings = [NLLanguage.english, .french].compactMap { NLEmbedding.wordEmbedding(for: $0) }
        self.init(vocabularies: embeddings.map { embedding in { embedding.contains($0) } }, expected: 2)
    }

    /// `vocabularies`: whether each language knows a word; `expected`: how many there should be.
    init(vocabularies: [(String) -> Bool], expected: Int) {
        self.vocabularies = vocabularies
        complete = vocabularies.count == expected
    }

    public func similar(_ a: String, _ b: String) -> Bool {
        !rareTokens(a).isDisjoint(with: rareTokens(b))
    }

    func rareTokens(_ text: String) -> Set<String> {
        let tokens = text.lowercased()
            .split { !($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_" || $0 == ".") }
            .map { String($0).trimmingCharacters(in: CharacterSet(charactersIn: "-_.")) }
            .filter { $0.count >= 4 && $0.contains(where: \.isLetter) && !KeywordSimilarity.stopWords.contains($0) }
        let names = Self.personalNames(text)
        return Set(
            tokens.filter { token in
                if token.contains(where: { $0.isNumber || $0 == "-" || $0 == "_" }) { return true }
                return complete && !known(token) && !isTypo(token) && !names.contains(token)
            })
    }

    private func known(_ word: String) -> Bool { vocabularies.contains { $0(word) } }

    /// One edit away from a known word: a letter changed, added or dropped, or two neighbours swapped.
    func isTypo(_ word: String) -> Bool {
        lock.lock()
        if let answer = typos[word] {
            lock.unlock()
            return answer
        }
        lock.unlock()
        let letters = Array(word)
        let alphabet = Array("abcdefghijklmnopqrstuvwxyzàâçéèêëîïôûùüœ")
        var candidates: [String] = []
        for i in letters.indices {
            candidates.append(String(letters[..<i] + letters[(i + 1)...]))
            if i + 1 < letters.count {
                var swapped = letters
                swapped.swapAt(i, i + 1)
                candidates.append(String(swapped))
            }
            for c in alphabet where c != letters[i] {
                var changed = letters
                changed[i] = c
                candidates.append(String(changed))
            }
        }
        for i in 0...letters.count {
            for c in alphabet { candidates.append(String(letters[..<i] + [c] + letters[i...])) }
        }
        let answer = candidates.contains { $0.count >= 4 && known($0) }
        lock.lock()
        typos[word] = answer
        lock.unlock()
        return answer
    }

    /// The people Apple's on-device tagger names in a title, lowercased.
    static func personalNames(_ text: String) -> Set<String> {
        let tagger = NLTagger(tagSchemes: [.nameType])
        tagger.string = text
        var names: Set<String> = []
        tagger.enumerateTags(
            in: text.startIndex..<text.endIndex, unit: .word, scheme: .nameType,
            options: [.omitPunctuation, .omitWhitespace]
        ) { tag, range in
            if tag == .personalName { names.insert(text[range].lowercased()) }
            return true
        }
        return names
    }
}

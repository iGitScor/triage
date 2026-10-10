import Foundation

/// What a chat message asks of you.
public enum TextIntent: Equatable, Sendable {
    /// A question or request: someone waits for your answer.
    case request
    /// Information you may read, nothing to answer.
    case info
}

/// Reads a message and guesses its intent. Returns nil when unsure.
public protocol TextIntentClassifier: Sendable {
    func intent(of text: String) -> TextIntent?
}

/// Moves items from the kind a plugin reported to the verb you have to perform.
/// Runs on every refresh, entirely on device.
public struct VerbClassifier: Sendable {
    private let classifiers: [any TextIntentClassifier]

    /// Classifiers are tried in order; the first confident one wins.
    public init(_ classifiers: [any TextIntentClassifier] = [KeywordIntentClassifier()]) {
        self.classifiers = classifiers
    }

    public func classify(_ item: InboxItem) -> InboxItem {
        var item = item
        switch item.bundle.id {
        case InboxBundle.authored.id:
            item.bundle = verbForAuthored(item)
            item.needsAction = item.bundle != .awaiting
        case InboxBundle.directMessages.id, InboxBundle.mentions.id:
            let fallback: TextIntent = .request
            let intent = intent(of: [item.title, item.preview].compactMap { $0 }.joined(separator: "\n")) ?? fallback
            item.bundle = intent == .request ? .reply : .read
            item.needsAction = intent == .request
        default:
            break
        }
        return item
    }

    public func intent(of text: String) -> TextIntent? {
        classifiers.lazy.compactMap { $0.intent(of: text) }.first
    }

    private func verbForAuthored(_ item: InboxItem) -> InboxBundle {
        if item.hasBadge("draft") { return .awaiting }
        if item.hasBadge("changes") || item.hasBadge("checks.failing") || item.hasBadge("conflicts") { return .fix }
        if item.hasBadge("approved") { return .merge }
        return .awaiting
    }
}

/// Explicit wording in English and French. Requests win over FYI ("FYI, can you check?"), but a negated request
/// asks nothing: "no need to reply" is information, and "not urgent" cancels "urgent". A request said
/// elsewhere in the message still counts ("no need to reply, but can you check?").
/// Keywords match whole words, a question mark only ends a sentence, and links and code are left out:
/// "pleased", `a ?? b` and `search?q=x` ask nothing.
public struct KeywordIntentClassifier: TextIntentClassifier {
    static let requests = [
        "can you", "could you", "would you", "will you", "please", "pls", "plz", "let me know", "lmk",
        "what do you think", "thoughts", "your opinion", "need you", "waiting for you", "asap", "urgent",
        "peux-tu", "pourrais-tu", "tu peux", "tu pourrais", "pouvez-vous", "pourriez-vous", "vous pouvez",
        "merci de", "stp", "svp", "s'il te plaît", "s’il te plaît", "s'il vous plaît", "est-ce que",
        "qu'en penses", "qu’en penses", "dis-moi", "dites-moi", "ton avis", "votre avis", "besoin de toi",
    ]
    static let infos = [
        "fyi", "for your information", "heads up", "heads-up", "just so you know", "announcement",
        "@here", "@channel", "@everyone", "pour info", "pour information", "pour rappel", "à titre d'info",
        "a titre d'info", "je vous informe", "annonce",
    ]

    /// Says that nothing is expected: information, and taken out before looking for requests.
    static let noReply = withApostrophes([
        "no need to reply", "no need to answer", "no need to respond", "no need to do anything", "no reply needed",
        "no response needed", "no reply necessary", "no action needed", "no action required", "nothing to do",
        "you don't need to reply", "you don't have to reply", "don't need to reply", "no need for a reply",
        "pas besoin de répondre", "pas besoin de me répondre", "pas besoin de réponse", "inutile de répondre",
        "pas la peine de répondre", "aucune action requise", "aucune action nécessaire", "rien à faire",
        "tu n'as pas besoin de répondre", "vous n'avez pas besoin de répondre",
    ])
    /// Takes the urgency out, nothing more: "not urgent, but can you look?" is still a request.
    static let notUrgent = withApostrophes([
        "not urgent", "nothing urgent", "no rush", "no hurry", "pas urgent", "rien d'urgent", "pas d'urgence",
        "sans urgence", "pas pressé", "pas de rush",
    ])

    public init() {}

    public func intent(of text: String) -> TextIntent? {
        var text = Self.prose(text.lowercased())
        let saysNoReply = Self.matches(Self.noReplyWords, text)
        text = Self.removing(Self.noReplyWords, from: text)
        text = Self.removing(Self.notUrgentWords, from: text)
        if Self.matches(Self.question, text) || Self.matches(Self.requestWords, text) { return .request }
        if saysNoReply || Self.matches(Self.infoWords, text) { return .info }
        return nil
    }

    /// Each phrase with a straight and a curly apostrophe, as people type both.
    static func withApostrophes(_ phrases: [String]) -> [String] {
        Array(Set(phrases + phrases.map { $0.replacingOccurrences(of: "'", with: "’") }))
    }

    static func removing(_ pattern: NSRegularExpression, from text: String) -> String {
        pattern.stringByReplacingMatches(in: text, range: NSRange(text.startIndex..., in: text), withTemplate: "$1 $3")
    }

    /// The text without links and code, which aren't wording.
    static func prose(_ text: String) -> String {
        text.replacingOccurrences(of: "```[\\s\\S]*?```", with: " ", options: .regularExpression)
            .replacingOccurrences(of: "`[^`]*`", with: " ", options: .regularExpression)
            .replacingOccurrences(of: "(https?://|www\\.)\\S+", with: " ", options: .regularExpression)
    }

    /// A question mark that ends a sentence: before a space, a closing quote or bracket, or the end.
    static let question = try! NSRegularExpression(pattern: #"\?+["'”’)\]]*(\s|$)"#)
    static let requestWords = wordPattern(requests)
    static let infoWords = wordPattern(infos)
    static let noReplyWords = wordPattern(noReply)
    static let notUrgentWords = wordPattern(notUrgent)

    /// Any of the keywords as whole words: not preceded or followed by a letter or a digit. One pattern, compiled once.
    static func wordPattern(_ keywords: [String]) -> NSRegularExpression {
        let alternatives = keywords.sorted { $0.count > $1.count }.map(NSRegularExpression.escapedPattern(for:)).joined(separator: "|")
        return try! NSRegularExpression(pattern: "(^|[^\\p{L}\\p{N}])(\(alternatives))($|[^\\p{L}\\p{N}])")
    }

    static func matches(_ pattern: NSRegularExpression, _ text: String) -> Bool {
        pattern.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)) != nil
    }
}

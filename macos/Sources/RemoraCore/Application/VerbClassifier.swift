import Foundation

/// What a chat message asks of you.
public enum TextIntent: Equatable, Sendable {
    /// A question or request: someone waits for your answer.
    case request
    /// Information you may read, nothing to answer.
    case info
    /// A thank-you, an OK, a 👍: the conversation is over, nobody waits for anything.
    case closing
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
            item.bundle = intent == .request ? .reply : intent == .closing ? .finished : .read
            item.needsAction = intent == .request
        case InboxBundle.asked.id, InboxBundle.answered.id:
            // Your own message, last in the conversation: a question or request waits on the others. Anything else
            // waits on nobody: an answer of yours finishes the conversation (Done), and a message nobody asked for
            // (an FYI) is cleared. Unsure means not a question, so the Waiting tab doesn't fill with chatter.
            let intent = intent(of: [item.title, item.preview].compactMap { $0 }.joined(separator: "\n"))
            let answered = item.bundle == .answered
            item.needsAction = false
            if intent == .request {
                item.bundle = .awaiting
            } else if answered {
                item.bundle = .finished
            } else {
                item.bundle = .awaiting
                item.expires = item.date
            }
        default:
            break
        }
        return item
    }

    public func intent(of text: String) -> TextIntent? {
        classifiers.lazy.compactMap { $0.intent(of: text) }.first
    }

    private func verbForAuthored(_ item: InboxItem) -> InboxBundle {
        if item.hasBadge(.draft) { return .awaiting }
        if item.hasBadge(.changes) || item.hasBadge(.checksFailing) || item.hasBadge(.conflicts) { return .fix }
        if item.hasBadge(.approved) { return .merge }
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
        "stp", "svp", "s'il te plaît", "s’il te plaît", "s'il vous plaît", "est-ce que",
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
    /// Ends a conversation when a short message says little else: "thanks!", "sounds good", "merci, parfait". After
    /// requests and explicit information, which win.
    static let closers = withApostrophes([
        "thanks", "thank you", "thx", "ty", "tysm", "cheers", "great", "perfect", "awesome", "sounds good", "sgtm",
        "got it", "ok", "okay", "kk", "noted", "will do", "all good", "no problem", "np", "cool", "nice",
        "lgtm", "ack", "roger", "appreciated", "merci", "merci beaucoup", "parfait", "super", "top", "ça marche",
        "ca marche", "d'accord", "dac", "entendu", "noté", "bien reçu", "c'est noté", "nickel", "génial",
        "impeccable", "pas de souci", "pas de soucis", "ça roule", "bien vu", "vu",
    ])
    /// The most words a closing message has: "Thanks a lot, that's perfect for me" closes, a paragraph doesn't.
    static let closingWords = 12

    /// Takes the urgency out, nothing more: "not urgent, but can you look?" is still a request.
    static let notUrgent = withApostrophes([
        "not urgent", "nothing urgent", "no rush", "no hurry", "pas urgent", "rien d'urgent", "pas d'urgence",
        "sans urgence", "pas pressé", "pas de rush",
    ])

    public init() {}

    public func intent(of text: String) -> TextIntent? {
        let lowercased = text.lowercased()
        var text = Self.prose(lowercased)
        // Nothing but code or links (a snippet, a URL pasted alone) asks nothing in words: to read (ML-05).
        if text.allSatisfy(\.isWhitespace) { return lowercased.allSatisfy(\.isWhitespace) ? nil : .info }
        let saysNoReply = Self.matches(Self.noReplyWords, text)
        text = Self.removing(Self.noReplyWords, from: text)
        text = Self.removing(Self.notUrgentWords, from: text)
        if Self.matches(Self.question, text) || Self.matches(Self.requestWords, text) || Self.asksWithMerci(text) {
            return .request
        }
        // Explicit information wins over a closing word: "FYI, done", "nothing to do, thanks" are to read.
        if saysNoReply || Self.matches(Self.infoWords, text) { return .info }
        if Self.closes(text) { return .closing }
        return nil
    }

    /// "Merci de" and "merci d'" before something to do ask for it ("merci de relire", "merci d'accepter", "merci de
    /// bien vouloir signer", "merci de ne pas merger"); before what was given or done they thank ("merci de ton
    /// retour", "merci d'avoir regardé", "merci d'avance"). Something to do is an infinitive, after its pronouns.
    static func asksWithMerci(_ text: String) -> Bool {
        merciDe.matches(in: text, range: NSRange(text.startIndex..., in: text)).contains { match in
            guard let range = Range(match.range(at: 3), in: text) else { return false }
            let words = text[range].split { !$0.isLetter }.map(String.init)
            guard let word = words.first(where: { !merciSkipped.contains($0) }) else { return false }
            return word == "ne"
                || (["er", "ir", "re", "oir"].contains { word.hasSuffix($0) }
                    && !["avoir", "être", "etre"].contains(word) && !determiners.contains(word))
        }
    }

    static let merciDe = try! NSRegularExpression(
        pattern: #"(^|[^\p{L}\p{N}])merci (de\s+|d['’])((\p{L}+['’]?\s*){1,3})"#)
    /// Pronouns and articles before the word that tells: "merci de me renvoyer", "merci de la relire".
    static let merciSkipped: Set<String> = [
        "me", "m", "te", "t", "nous", "vous", "lui", "leur", "le", "la", "les", "l", "se", "s", "y", "en", "bien",
    ]
    static let determiners: Set<String> = ["votre", "notre", "vos", "nos", "ton", "ta", "tes", "mon", "ma", "mes"]

    /// A short message with a closing word, or nothing but emoji and punctuation (a lone 👍).
    static func closes(_ text: String) -> Bool {
        let words = text.replacingOccurrences(of: ":[a-z0-9_+'-]+:", with: " ", options: .regularExpression)
            .split { !$0.isLetter && !$0.isNumber && $0 != "'" && $0 != "’" && $0 != "-" }
        if words.isEmpty { return !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
        return words.count <= closingWords && matches(closerWords, text)
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
    static let closerWords = wordPattern(closers)

    /// Any of the keywords as whole words: not preceded or followed by a letter or a digit. One pattern, compiled once.
    static func wordPattern(_ keywords: [String]) -> NSRegularExpression {
        let alternatives = keywords.sorted { $0.count > $1.count }.map(NSRegularExpression.escapedPattern(for:)).joined(
            separator: "|")
        return try! NSRegularExpression(pattern: "(^|[^\\p{L}\\p{N}])(\(alternatives))($|[^\\p{L}\\p{N}])")
    }

    static func matches(_ pattern: NSRegularExpression, _ text: String) -> Bool {
        pattern.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)) != nil
    }
}

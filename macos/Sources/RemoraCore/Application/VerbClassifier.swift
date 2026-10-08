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

/// Explicit wording in English and French. Requests win over FYI ("FYI, can you check?").
public struct KeywordIntentClassifier: TextIntentClassifier {
    static let requests = [
        "?", "can you", "could you", "would you", "will you", "please", "pls", "plz", "let me know", "lmk",
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

    public init() {}

    public func intent(of text: String) -> TextIntent? {
        let text = text.lowercased()
        if Self.requests.contains(where: text.contains) { return .request }
        if Self.infos.contains(where: text.contains) { return .info }
        return nil
    }
}

import Foundation

/// Why a message went where it is: the words that decided, the model, nothing clear, or you. Shown on the row, read by
/// VoiceOver, and given to agents, so a strong signal can be told from a guess. Set on chat messages, the items
/// Remora sorts by their text; a source's own sort (a review requested, failing checks) is told by its badges.
/// Open, not an enum: a saved inbox holding a code this version doesn't know still loads.
public struct SortReason: Hashable, Codable, Sendable {
    public var code: Code
    /// The words that decided, as found in the message ("can you", "merci de", "fyi"), for `keyword`, `noReply`,
    /// `info` and `closing`.
    public var detail: String?

    public init(_ code: Code, _ detail: String? = nil) {
        self.code = code
        self.detail = detail
    }

    public struct Code: RawRepresentable, Hashable, Codable, Sendable {
        public let rawValue: String
        public init(rawValue: String) { self.rawValue = rawValue }

        public init(from decoder: any Decoder) throws {
            rawValue = try decoder.singleValueContainer().decode(String.self)
        }

        public func encode(to encoder: any Encoder) throws {
            var container = encoder.singleValueContainer()
            try container.encode(rawValue)
        }

        /// A question mark ends a sentence.
        public static let question = Code(rawValue: "question")
        /// Words that ask: "can you", "peux-tu", "merci de relire".
        public static let keyword = Code(rawValue: "keyword")
        /// Words that say nothing is expected: "no need to reply".
        public static let noReply = Code(rawValue: "noReply")
        /// Words that announce: "fyi", "@here", "pour info".
        public static let info = Code(rawValue: "info")
        /// A short thank-you or OK, or nothing but emoji.
        public static let closing = Code(rawValue: "closing")
        /// Nothing but code or a link.
        public static let codeOnly = Code(rawValue: "codeOnly")
        /// No words decided; the model read the meaning (the Mac's sentence embeddings).
        public static let model = Code(rawValue: "model")
        /// Nothing clear: a message to you stays in To reply to be safe, a message of yours isn't taken for a question.
        public static let unsure = Code(rawValue: "unsure")
        /// You put it in To reply.
        public static let moved = Code(rawValue: "moved")
    }
}

extension SortReason {
    /// What the row says, in the interface's language, and VoiceOver with it: only a guess or the user's correction.
    /// Clear words ("can you", "fyi") or the model's reading go without saying on the row; agents get every reason.
    /// `bundle` is where the item is now: a guess kept in To reply says it was kept there to be safe.
    public func label(in bundle: InboxBundle) -> String? {
        switch code {
        case .unsure: bundle == .reply ? L("No clear sign, kept here to be safe") : L("No clear sign")
        case .moved: L("You put it here")
        default: nil
        }
    }

    public var symbol: String { code == .moved ? "hand.point.right" : "questionmark.circle" }
}

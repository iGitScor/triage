import Foundation

/// An emoji reaction on the message an item comes from (Slack). Not part of the item's fingerprint: a new 👍
/// neither brings a Done item back nor wakes a snooze.
public struct Reaction: Hashable, Codable, Sendable {
    /// The emoji's name as the tool writes it (`white_check_mark`), skin tone dropped.
    public var name: String
    public var count: Int
    /// Whether you are among the people who reacted.
    public var mine: Bool
    /// A workspace's own emoji has no character, only a picture.
    public var image: URL?

    public init(name: String, count: Int, mine: Bool, image: URL? = nil) {
        self.name = Self.base(name)
        self.count = max(count, mine ? 1 : 0)
        self.mine = mine
        self.image = image
    }

    /// 👍, or `:name:` for a workspace's own emoji (shown as its `image` where pictures may load).
    public var emoji: String { Emoji.table[name] ?? ":\(name):" }

    public var meaning: ReactionMeaning? { ReactionMeaning.of(name) }

    /// How many people other than you reacted.
    public var others: Int { count - (mine ? 1 : 0) }

    /// `+1::skin-tone-2` → `+1`.
    static func base(_ name: String) -> String {
        name.components(separatedBy: "::").first ?? name
    }

    /// One reaction per emoji: skin tones of the same emoji are added up.
    public static func merged(_ reactions: [Reaction]) -> [Reaction] {
        var order: [String] = []
        var byName: [String: Reaction] = [:]
        for reaction in reactions {
            if var existing = byName[reaction.name] {
                existing.count += reaction.count
                existing.mine = existing.mine || reaction.mine
                byName[reaction.name] = existing
            } else {
                order.append(reaction.name)
                byName[reaction.name] = reaction
            }
        }
        return order.compactMap { byName[$0] }
    }
}

/// What a reaction says about the message (the list is in `scripts/make-emoji.py`).
public enum ReactionMeaning: String, Codable, Sendable, CaseIterable {
    /// ✅: handled. Yours moves the item to Done; theirs on your question answers it.
    case done
    /// 👀: being looked at. Yours moves the item to In progress.
    case started
    /// 👍: seen, fine.
    case ack
    /// ❓: they need something from you.
    case question

    /// A standard emoji's meaning, else a word of a workspace emoji's name (`:approved:`, `:merci-beaucoup:`).
    public static func of(_ name: String) -> ReactionMeaning? {
        let name = name.lowercased()
        if let meaning = Emoji.meanings[name] { return meaning }
        let words = Set(name.split { $0 == "_" || $0 == "-" || $0 == "+" }.map(String.init) + [name])
        return Emoji.meaningWords.first { !$0.1.isDisjoint(with: words) }?.0
    }
}

extension InboxItem {
    /// What your own reaction says, if it says something: handled beats being looked at.
    public var yourReaction: ReactionMeaning? {
        let yours = Set((reactions ?? []).filter(\.mine).compactMap(\.meaning))
        if yours.contains(.done) { return .done }
        return yours.contains(.started) ? .started : nil
    }

    /// Whether someone other than you reacted with this meaning.
    public func othersReacted(_ meaning: ReactionMeaning) -> Bool {
        (reactions ?? []).contains { $0.others > 0 && $0.meaning == meaning }
    }

    /// The reactions as the assistant reads them: meanings and counts, never who.
    public var reactionNotes: [String] {
        var notes: [String] = []
        if let yours = yourReaction { notes.append("you reacted: \(yours.rawValue)") }
        for meaning in ReactionMeaning.allCases {
            let count = (reactions ?? []).filter { $0.meaning == meaning }.map(\.others).reduce(0, +)
            if count > 0 { notes.append("others reacted: \(meaning.rawValue) ×\(count)") }
        }
        return notes
    }
}

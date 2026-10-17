import Foundation

/// The `remora` command: what a launcher, a status bar or a script may ask the running app, over a socket
/// only this user can open (`LocalSocket`). The count, a tab's items, a new reminder: never Done, a snooze or anything
/// that removes. Items whose tool hides message content come without their title. One JSON object per line each way.
/// Same rule on Windows and Linux (`command.rs`), with the same tests.
public enum RemoraCommand {
    /// A request longer than this is refused: it can only be a mistake.
    public static let lineLimit = 4096
    public static let usage = """
        Usage: remora count [--json]
               remora list [--tab in-progress|my-turn|waiting|snoozed|done] [--json]
               remora remind "Call Erin" --at 15:00 [--json]
        Asks the Remora app running on this computer; nothing goes over the network.
        """
    /// The tabs, as links name them (`RemoraLink`).
    static let tabs = ["in-progress", "my-turn", "waiting", "snoozed", "done"]

    /// Why the app refused, in English.
    public struct Refusal: Error, Equatable {
        public let message: String
        public init(_ message: String) { self.message = message }
    }

    // MARK: The app's side

    /// What the app answers one request line with: a JSON object, `"ok": false` with an `error` when refused.
    /// `hides`: whether a tool's message content is hidden (Settings, or the organization's policy). `remind` saves a
    /// reminder and returns its id.
    public static func answer(
        _ line: String, layout: InboxLayout, hides: (String) -> Bool, now: Date = .now,
        remind: (String, Date) throws(Refusal) -> String
    ) -> String {
        do {
            return encode(try respond(line, layout: layout, hides: hides, now: now, remind: remind))
        } catch {
            return encode(["ok": false, "error": error.message])
        }
    }

    private static func respond(
        _ line: String, layout: InboxLayout, hides: (String) -> Bool, now: Date,
        remind: (String, Date) throws(Refusal) -> String
    ) throws(Refusal) -> [String: Any] {
        let unknown = Refusal("Not a request Remora knows.")
        guard line.utf8.count <= lineLimit,
            let data = line.trimmingCharacters(in: .whitespacesAndNewlines).data(using: .utf8),
            let request = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
        else { throw unknown }
        let text = { (name: String) in request[name] as? String }
        switch text("command") {
        case "count":
            let counts = Dictionary(uniqueKeysWithValues: tabs.map { ($0, items(layout, tab: $0).count) })
            return ["ok": true, "count": layout.actionCount, "tabs": counts]
        case "list":
            let tab = (text("tab") ?? "my-turn").trimmingCharacters(in: .whitespaces).lowercased()
            guard tabs.contains(tab) else {
                throw Refusal("No tab \(tab): in-progress, my-turn, waiting, snoozed or done.")
            }
            return ["ok": true, "tab": tab, "items": items(layout, tab: tab).map { row($0, hides: hides) }]
        case "remind":
            let title = String(
                (text("text") ?? "").trimmingCharacters(in: .whitespacesAndNewlines).prefix(RemoraLink.textLimit))
            guard !title.isEmpty else { throw Refusal("Type what to remember.") }
            guard let at = time(text("at") ?? "", now: now) else {
                throw Refusal("Say when: 15:00, or 2026-10-14T09:00.")
            }
            guard at > now else { throw Refusal("That time has passed.") }
            let id = try remind(title, at)
            let formatter = ISO8601DateFormatter()
            return ["ok": true, "id": id, "at": formatter.string(from: at)]
        default: throw unknown
        }
    }

    static func items(_ layout: InboxLayout, tab: String) -> [InboxItem] {
        switch tab {
        case "in-progress": layout.inProgress
        case "my-turn": layout.myTurnItems
        case "waiting": layout.waitingItems
        case "snoozed": layout.snoozed
        default: layout.done
        }
    }

    /// Where and what kind, always; what was written (the title) only when the tool's content isn't hidden. `why`:
    /// why Remora sorted a message there (`SortReason`), its words left out when the content is hidden; null for what
    /// the source sorted.
    static func row(_ item: InboxItem, hides: (String) -> Bool) -> [String: Any] {
        let hidden = hides(item.pluginID)
        let why: Any =
            item.reason.map { reason -> Any in
                ["code": reason.code.rawValue, "detail": hidden ? NSNull() as Any : reason.detail ?? NSNull() as Any]
            } ?? NSNull()
        return [
            "id": item.id, "source": item.pluginID, "kind": item.bundle.title,
            "title": hidden ? NSNull() as Any : item.title, "context": item.context,
            "url": item.url.map { $0.absoluteString as Any } ?? NSNull(), "hidden": hidden, "why": why,
        ]
    }

    /// "15:00": the next 15:00 (tomorrow once it has passed). Otherwise as a link has it: "2026-10-14T09:00" on this
    /// Mac's clock, or with its offset.
    public static func time(_ raw: String, now: Date) -> Date? {
        let raw = raw.trimmingCharacters(in: .whitespaces)
        let parts = raw.split(separator: ":", omittingEmptySubsequences: false)
        guard parts.count == 2, (1...2).contains(parts[0].count), parts[1].count == 2,
            let hour = Int(parts[0]), let minute = Int(parts[1]), (0..<24).contains(hour), (0..<60).contains(minute)
        else { return RemoraLink.time(raw) }
        let calendar = Calendar.current
        guard let today = calendar.date(bySettingHour: hour, minute: minute, second: 0, of: now) else { return nil }
        return today > now ? today : calendar.date(byAdding: .day, value: 1, to: today)
    }

    private static func encode(_ object: [String: Any]) -> String {
        let data = try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys, .withoutEscapingSlashes])
        return data.flatMap { String(data: $0, encoding: .utf8) } ?? #"{"ok":false}"#
    }

    // MARK: The command's side

    /// What the command line asks: the request line to send, and whether to print the answer as JSON.
    public struct Invocation: Equatable {
        public var request: String
        public var json: Bool
    }

    /// `remora count`, `remora list --tab waiting --json`, `remora remind Call Erin --at 15:00`. Nil: show the usage.
    public static func invocation(_ args: [String]) -> Invocation? {
        guard let command = args.first else { return nil }
        var json = false
        var words: [String] = []
        var options: [String: Any] = [:]
        var rest = args.dropFirst().makeIterator()
        while let arg = rest.next() {
            if arg == "--json" {
                json = true
            } else if arg.hasPrefix("--") {
                let option = arg.dropFirst(2)
                if let equals = option.firstIndex(of: "=") {
                    options[String(option[..<equals])] = String(option[option.index(after: equals)...])
                } else {
                    guard let value = rest.next() else { return nil }
                    options[String(option)] = value
                }
            } else {
                words.append(arg)
            }
        }
        let allowed: [String]
        switch command {
        case "count": allowed = []
        case "list": allowed = ["tab"]
        case "remind": allowed = ["at"]
        default: return nil
        }
        guard command == "remind" || words.isEmpty, options.keys.allSatisfy(allowed.contains) else { return nil }
        options["command"] = command
        if command == "remind" { options["text"] = words.joined(separator: " ") }
        return Invocation(request: encode(options), json: json)
    }

    /// What the command prints for the app's answer: (standard output, standard error, exit code). JSON prints the
    /// answer as it came; otherwise a number, a line per item, or a sentence. 1: the app refused.
    public static func output(_ response: String, json: Bool) -> (out: String, err: String, code: Int32) {
        guard let data = response.trimmingCharacters(in: .whitespacesAndNewlines).data(using: .utf8),
            let value = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
        else { return ("", "Remora answered something this command doesn't understand.", 1) }
        let ok = value["ok"] as? Bool == true
        if json { return (encode(value), "", ok ? 0 : 1) }
        guard ok else { return ("", value["error"] as? String ?? "Remora refused.", 1) }
        if let count = value["count"] as? Int { return (String(count), "", 0) }
        if let items = value["items"] as? [[String: Any]] {
            guard !items.isEmpty else { return ("Nothing here.", "", 0) }
            let lines = items.map { item in
                let title = item["title"] as? String ?? "(content hidden)"
                return "\(item["kind"] as? String ?? "") · \(item["context"] as? String ?? "") · \(title)"
            }
            return (lines.joined(separator: "\n"), "", 0)
        }
        if let raw = value["at"] as? String, let at = Date(iso8601: raw) {
            let formatter = DateFormatter()
            formatter.locale = Locale(identifier: "en_US_POSIX")
            formatter.dateFormat = "yyyy-MM-dd HH:mm"
            return ("Reminder set for \(formatter.string(from: at)).", "", 0)
        }
        return ("", "", 0)
    }
}

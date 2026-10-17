import Foundation
import Testing

@testable import RemoraCore

/// The same cases as Windows and Linux (`command.rs`).
struct RemoraCommandTests {
    let now = Calendar.current.date(from: DateComponents(year: 2026, month: 10, day: 15, hour: 12))!

    func local(_ day: Int, _ hour: Int) -> Date {
        Calendar.current.date(from: DateComponents(year: 2026, month: 10, day: day, hour: hour))!
    }

    func item(_ id: String, _ plugin: String, _ bundle: InboxBundle, _ title: String, _ context: String) -> InboxItem {
        InboxItem(
            id: id, accountID: account, pluginID: plugin, bundle: bundle, title: title, context: context,
            url: URL(string: "https://example.com/\(id)"), date: now)
    }

    /// Two items your turn (one from Slack, whose content is hidden), one waiting, one done.
    var layout: InboxLayout {
        var layout = InboxLayout()
        layout.myTurn = [
            .init(bundle: .reviews, items: [item("1", "github", .reviews, "Fix login", "acme/app #12")]),
            .init(bundle: .mentions, items: [item("2", "slack", .mentions, "Salary review at 3?", "#general")]),
        ]
        layout.waiting = [.init(bundle: .awaiting, items: [item("3", "gitlab", .awaiting, "Bump deps", "acme/api !4")])]
        layout.done = [item("4", "linear", .reviews, "Ship it", "ENG-1")]
        return layout
    }

    final class Saved { var reminders: [(String, Date)] = [] }

    func ask(_ line: String) -> ([String: Any], Saved) {
        let saved = Saved()
        let answer = RemoraCommand.answer(
            line, layout: layout, hides: { $0 == "slack" }, now: now,
            remind: { text, at in
                saved.reminders.append((text, at))
                return "reminder/1"
            })
        let object = try! JSONSerialization.jsonObject(with: Data(answer.utf8)) as! [String: Any]
        return (object, saved)
    }

    func items(_ answer: [String: Any]) -> [[String: Any]] { answer["items"] as? [[String: Any]] ?? [] }

    @Test func countsWhatNeedsYouAndEachTab() {
        let (answer, _) = ask(#"{"command":"count"}"#)
        #expect(answer["ok"] as? Bool == true)
        #expect(answer["count"] as? Int == 2)
        #expect(
            answer["tabs"] as? [String: Int] == ["in-progress": 0, "my-turn": 2, "waiting": 1, "snoozed": 0, "done": 1])
    }

    @Test func listsATabWithoutTheHiddenContent() {
        let (answer, _) = ask(#"{"command":"list"}"#)
        #expect(answer["tab"] as? String == "my-turn", "your turn unless told")
        let rows = items(answer)
        #expect(rows.count == 2)
        #expect(rows[0]["title"] as? String == "Fix login")
        #expect(rows[0]["kind"] as? String == "To review")
        #expect(rows[0]["url"] as? String == "https://example.com/1")
        #expect(rows[0]["hidden"] as? Bool == false)
        #expect(rows[1]["title"] is NSNull, "Slack's content is hidden")
        #expect(rows[1]["context"] as? String == "#general", "where, not what")
        #expect(rows[1]["hidden"] as? Bool == true)
        #expect(!"\(answer)".contains("Salary"))

        #expect(items(ask(#"{"command":"list","tab":"Waiting"}"#).0).first?["title"] as? String == "Bump deps")
        let (nowhere, _) = ask(#"{"command":"list","tab":"mine"}"#)
        #expect(nowhere["ok"] as? Bool == false)
        #expect(nowhere["error"] as? String == "No tab mine: in-progress, my-turn, waiting, snoozed or done.")
    }

    @Test func savesAReminderAtTheNextSuchTime() {
        let (answer, saved) = ask(#"{"command":"remind","text":"  Call Erin ","at":"15:00"}"#)
        #expect(saved.reminders.map(\.0) == ["Call Erin"])
        #expect(saved.reminders.map(\.1) == [local(15, 15)])
        #expect(answer["id"] as? String == "reminder/1")
        #expect(answer["at"] as? String == ISO8601DateFormatter().string(from: local(15, 15)))
        #expect(ask(#"{"command":"remind","text":"Stand-up","at":"09:00"}"#).1.reminders.first?.1 == local(16, 9))
        #expect(ask(#"{"command":"remind","text":"x","at":"2026-10-20T09:00"}"#).1.reminders.first?.1 == local(20, 9))
        let long = #"{"command":"remind","text":"\#(String(repeating: "a", count: 500))","at":"15:00"}"#
        #expect(ask(long).1.reminders.first?.0.count == RemoraLink.textLimit)

        for (line, error) in [
            (#"{"command":"remind","text":" ","at":"15:00"}"#, "Type what to remember."),
            (#"{"command":"remind","text":"x"}"#, "Say when: 15:00, or 2026-10-14T09:00."),
            (#"{"command":"remind","text":"x","at":"2026-10-01T09:00"}"#, "That time has passed."),
        ] {
            let (answer, saved) = ask(line)
            #expect(answer["ok"] as? Bool == false, "\(line)")
            #expect(answer["error"] as? String == error, "\(line)")
            #expect(saved.reminders.isEmpty)
        }
    }

    /// Nothing marks an item done, snoozes it or removes anything.
    @Test func refusesAnythingElse() {
        let long = #"{"command":"count","pad":"\#(String(repeating: "a", count: RemoraCommand.lineLimit))"}"#
        for line in [#"{"command":"done","id":"1"}"#, #"{"command":"snooze"}"#, "count", "", "[]", long] {
            let (answer, saved) = ask(line)
            #expect(answer["ok"] as? Bool == false, "\(line.prefix(40))")
            #expect(answer["error"] as? String == "Not a request Remora knows.")
            #expect(saved.reminders.isEmpty)
        }
    }

    @Test func readsTheCommandLine() {
        func request(_ line: String) -> ([String: String], Bool)? {
            let args = line.split(separator: " ").map(String.init)
            guard let invocation = RemoraCommand.invocation(args) else { return nil }
            let object = try! JSONSerialization.jsonObject(with: Data(invocation.request.utf8)) as! [String: String]
            return (object, invocation.json)
        }
        #expect(request("count")?.0 == ["command": "count"])
        #expect(request("count")?.1 == false)
        #expect(request("list --tab waiting --json")?.0 == ["command": "list", "tab": "waiting"])
        #expect(request("list --tab waiting --json")?.1 == true)
        #expect(request("list --tab=done")?.0 == ["command": "list", "tab": "done"])
        #expect(request("remind Call Erin --at 15:00")?.0 == ["command": "remind", "text": "Call Erin", "at": "15:00"])
        for wrong in ["", "help", "done 1", "list --tab", "list --at 15:00", "count extra", "list waiting"] {
            #expect(request(wrong) == nil, "\(wrong)")
        }
    }

    @Test func printsANumberALinePerItemOrASentence() {
        func print(_ line: String, json: Bool = false) -> (out: String, err: String, code: Int32) {
            let answer = RemoraCommand.answer(
                line, layout: layout, hides: { $0 == "slack" }, now: now, remind: { _, _ in "reminder/1" })
            return RemoraCommand.output(answer, json: json)
        }
        #expect(print(#"{"command":"count"}"#) == ("2", "", 0))
        #expect(
            print(#"{"command":"list"}"#).out
                == "To review · acme/app #12 · Fix login\nMentions · #general · (content hidden)")
        #expect(print(#"{"command":"list","tab":"snoozed"}"#).out == "Nothing here.")
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd HH:mm"
        #expect(
            print(#"{"command":"remind","text":"x","at":"15:00"}"#).out
                == "Reminder set for \(formatter.string(from: local(15, 15))).")
        #expect(print(#"{"command":"list","tab":"x"}"#).code == 1)
        #expect(print(#"{"command":"done"}"#) == ("", "Not a request Remora knows.", 1))
        let json = print(#"{"command":"count"}"#, json: true)
        let object = try? JSONSerialization.jsonObject(with: Data(json.out.utf8)) as? [String: Any]
        #expect(object?["count"] as? Int == 2 && json.code == 0)
        #expect(RemoraCommand.output("garbage", json: false).code == 1)
    }

    /// Both ends over a real socket, which only this user may open.
    @Test func asksAndAnswersOverAPrivateSocket() throws {
        let path = NSTemporaryDirectory() + "remora-\(UUID().uuidString.prefix(8)).sock"
        let server = try LocalSocketServer(path: path) { line in #"{"ok":true,"echo":\#(line)}"# }
        defer { server.stop() }
        let mode = try FileManager.default.attributesOfItem(atPath: path)[.posixPermissions] as? Int
        #expect(mode == 0o600)
        #expect(try LocalSocket.ask(#"{"command":"count"}"#, at: path) == #"{"ok":true,"echo":{"command":"count"}}"#)
        #expect(throws: (any Error).self, "one app per socket") { try LocalSocketServer(path: path) { _ in "" } }
        #expect(throws: (any Error).self, "nothing listens: Remora isn't running") {
            try LocalSocket.ask("{}", at: NSTemporaryDirectory() + "none-\(UUID().uuidString.prefix(8)).sock")
        }
        #expect(LocalSocket.path(environment: ["REMORA_SOCKET": "/tmp/x.sock"]) == "/tmp/x.sock")
        #expect(LocalSocket.path(environment: [:]).hasSuffix("/remora.sock"))
    }
}

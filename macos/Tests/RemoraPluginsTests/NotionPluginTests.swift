import Foundation
import RemoraCore
import Testing

@testable import RemoraPlugins

struct NotionPluginTests {
    @Test func extractsDatabaseIDFromLinks() {
        let id = "0123456789abcdef0123456789abcdef"
        #expect(NotionPlugin.databaseID(from: "https://www.notion.so/acme/Tasks-\(id)?v=42") == id)
        #expect(NotionPlugin.databaseID(from: " 01234567-89ab-cdef-0123-456789abcdef ") == id)
    }

    @Test func listsOpenTasksOnly() async throws {
        let page = { (id: String, status: String) in
            """
            {"id": "\(id)", "url": "https://notion.so/\(id)", "last_edited_time": "2026-10-08T10:00:00.000Z",
             "properties": {
               "Name": {"type": "title", "title": [{"plain_text": "Task \(id)"}]},
               "Status": {"type": "status", "status": {"name": "\(status)"}},
               "Due": {"type": "date", "date": {"start": "2020-01-01"}}
             }}
            """
        }
        let http = StubHTTP(routes: [
            "/users/me": #"{"id": "u1", "type": "person", "name": "Alice", "person": {"email": "alice@acme.example"}}"#,
            "/databases/0123456789abcdef0123456789abcdef":
                #"{"title": [{"plain_text": "Team tasks"}], "data_sources": [{"id": "ds1"}]}"#,
            "/data_sources/ds1": #"""
            {"properties": {"Status": {"name": "Status", "type": "status", "status": {
              "options": [{"id": "o1", "name": "Done"}], "groups": [{"name": "Complete", "option_ids": ["o1"]}]}}}}
            """#,
            "/data_sources/ds1/query": #"{"results": [\#(page("a", "In progress")), \#(page("b", "Done"))]}"#,
        ])
        let plugin = try NotionPlugin(
            config: config(["token": "secret", "databases": "0123456789abcdef0123456789abcdef", "doneValues": "Done"]),
            http: http
        )
        let snapshot = try await plugin.fetch()
        let items = snapshot.items
        #expect(snapshot.identity == "Alice")
        #expect(items.map(\.title) == ["Task a"])
        #expect(items.first?.context == "Team tasks")
        #expect(items.first?.hasBadge("overdue") == true)
    }

    /// Notion leaves finished tasks out, so they don't use up the 50 places (FEAT-08): every status in the Complete
    /// group, in any language, the done values, and a ticked Done checkbox.
    @Test func asksNotionForOpenTasksOnly() throws {
        let schema = try JSONDecoder.api(snakeCase: true).decode(
            NotionPlugin.Schema.self,
            from: Data(
                #"""
                {"properties": {
                  "État": {"name": "État", "type": "status", "status": {
                    "options": [{"id": "1", "name": "À faire"}, {"id": "2", "name": "Terminé"}, {"id": "3", "name": "Abandonné"}],
                    "groups": [{"name": "To-do", "option_ids": ["1"]}, {"name": "Complete", "option_ids": ["2"]}]}},
                  "Priority": {"name": "Priority", "type": "select", "select": {"options": [{"name": "Done"}]}},
                  "Done": {"name": "Done", "type": "checkbox", "checkbox": {}},
                  "Urgent": {"name": "Urgent", "type": "checkbox", "checkbox": {}}
                }}
                """#.utf8))
        let filters = schema.notDone(["abandonné"])
        #expect(
            filters == [
                .unticked(property: "Done"), .statusIsNot(property: "État", "Terminé"),
                .statusIsNot(property: "État", "Abandonné"),
            ])
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        let json = String(
            data: try encoder.encode(
                NotionPlugin.Filter.and([.people(property: "Assignee", contains: "u1")] + filters)),
            encoding: .utf8)
        #expect(json?.contains(#"{"people":{"contains":"u1"},"property":"Assignee"}"#) == true)
        #expect(json?.contains(#"{"property":"État","status":{"does_not_equal":"Terminé"}}"#) == true)
        #expect(json?.contains(#"{"checkbox":{"equals":false},"property":"Done"}"#) == true)
    }

    /// A priority select isn't the status, and an "Urgent" checkbox doesn't mean done; a "Done" checkbox does.
    @Test func readsTheStatusAndDoneCheckboxByTypeAndName() throws {
        let json = { (extra: String) in
            Data(
                """
                {"id": "p", "last_edited_time": "2026-10-08T10:00:00.000Z", "properties": {
                  "Name": {"type": "title", "title": [{"plain_text": "Ship it"}]},
                  "Priority": {"type": "select", "select": {"name": "Done"}},
                  "Urgent": {"type": "checkbox", "checkbox": true}\(extra)
                }}
                """.utf8)
        }
        let decoder = JSONDecoder.api(snakeCase: true)
        let open = try decoder.decode(NotionPlugin.Page.self, from: json(""))
        #expect(open.status == nil)
        #expect(!open.isDone(["done"]))
        let selectStatus = try decoder.decode(
            NotionPlugin.Page.self, from: json(#", "Status": {"type": "select", "select": {"name": "Done"}}"#))
        #expect(selectStatus.status == "Done")
        #expect(selectStatus.isDone(["done"]))
        let checked = try decoder.decode(
            NotionPlugin.Page.self, from: json(#", "Done": {"type": "checkbox", "checkbox": true}"#))
        #expect(checked.isDone(["archived"]))
    }

    /// An internal connection without an email would list everyone's tasks.
    @Test func refusesToListEveryonesTasks() async throws {
        let http = StubHTTP(routes: [
            "/users/me": #"{"id": "bot1", "type": "bot", "bot": {"owner": {"workspace": true}}}"#
        ])
        let plugin = try NotionPlugin(
            config: config(["token": "secret", "databases": "0123456789abcdef0123456789abcdef"]), http: http)
        await #expect(
            throws: HTTPError.api(
                L("Remora can’t tell which Notion user you are: add your Notion email to this account."))
        ) {
            try await plugin.fetch()
        }
    }
}

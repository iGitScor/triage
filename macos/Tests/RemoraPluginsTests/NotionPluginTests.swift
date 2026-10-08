import Foundation
import Testing
import RemoraCore
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
            "/databases/0123456789abcdef0123456789abcdef": #"{"title": [{"plain_text": "Team tasks"}], "data_sources": [{"id": "ds1"}]}"#,
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
}

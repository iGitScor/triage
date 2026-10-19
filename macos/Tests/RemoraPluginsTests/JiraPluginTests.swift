import Foundation
import RemoraCore
import Testing

@testable import RemoraPlugins

/// Answers Jira by path: who you are, then the two searches, told apart by their JQL, and Confluence's search.
struct JiraStub: HTTPClient {
    var myself: String
    var assigned: String
    var watched: String
    var expectedAuthorization: String
    var confluence = JiraPluginTests.confluence
    /// The assigned issues with a pull request still open (Cloud's development panel).
    var withOpenChanges = #"{"issues": []}"#

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        #expect(request.value(forHTTPHeaderField: "Authorization") == expectedAuthorization)
        let url = request.url!
        let jql = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first { $0.name == "jql" }?.value
        let body =
            url.path.hasSuffix("/wiki/rest/api/search")
            ? confluence
            : url.path.hasSuffix("/myself")
                ? myself
                : jql?.hasPrefix("watcher") == true
                    ? watched : jql?.contains("development[") == true ? withOpenChanges : assigned
        return (Data(body.utf8), HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: nil)!)
    }
}

struct JiraPluginTests {
    let now = Date()
    var recent: String { Self.jira(now.addingTimeInterval(-3_600)) }
    var old: String { Self.jira(now.addingTimeInterval(-10 * 86_400)) }

    /// Jira's own timestamp format: no colon in the offset.
    static func jira(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd'T'HH:mm:ss.SSSZ"
        return formatter.string(from: date)
    }

    var assigned: String {
        """
        {"issues": [
          {"id": "10001", "key": "OPS-7", "fields": {"summary": "Rotate the TLS certificates", "priority": {"name": "Highest"},
           "duedate": "2020-01-01", "status": {"name": "In Progress"}, "updated": "\(recent)"}},
          {"id": "10002", "key": "OPS-8", "fields": {"summary": "Tidy the runbook", "priority": {"name": "Low"},
           "status": {"name": "To Do"}, "updated": "\(recent)"}}
        ], "isLast": true}
        """
    }

    func watched(mentioning id: String) -> String {
        """
        {"issues": [{"id": "10003", "key": "OPS-9", "fields": {"summary": "Pager rota", "comment": {"comments": [
          {"id": "501", "author": {"accountId": "erin", "displayName": "Erin", "avatarUrls": {"48x48": "https://secure.gravatar.com/avatar/e"}},
           "created": "\(recent)", "body": {"type": "doc", "content": [{"type": "paragraph", "content": [
             {"type": "mention", "attrs": {"id": "\(id)", "text": "@Alice"}}, {"type": "text", "text": " can you take Friday?"}]}]}},
          {"id": "502", "author": {"accountId": "erin", "displayName": "Erin"}, "created": "\(old)",
           "body": {"type": "doc", "content": [{"type": "mention", "attrs": {"id": "\(id)"}}]}},
          {"id": "503", "author": {"accountId": "erin", "displayName": "Erin"}, "created": "\(recent)",
           "body": {"type": "doc", "content": [{"type": "text", "text": "No mention here"}]}},
          {"id": "504", "author": {"accountId": "\(id)", "displayName": "Alice"}, "created": "\(recent)",
           "body": {"type": "doc", "content": [{"type": "mention", "attrs": {"id": "\(id)"}}]}}
        ]}}}]}
        """
    }

    /// Confluence's search for `mention = currentUser()`: a page, a comment on it, and a page you changed last.
    static let confluence = """
        {"results": [
          {"content": {"id": "98304", "type": "page", "title": "Q4 launch plan",
            "history": {"lastUpdated": {"by": {"accountId": "erin", "displayName": "Erin", "profilePicture": {"path": "/wiki/aa-avatar/erin"}}}},
            "_links": {"webui": "/spaces/ENG/pages/98304/Q4+launch+plan"}},
           "excerpt": "@@@hl@@@@Alice@@@endhl@@@ can you review this page?", "lastModified": "2026-10-12T09:14:03.000Z",
           "resultGlobalContainer": {"title": "Engineering"}},
          {"content": {"id": "98400", "type": "comment", "title": "Re: Q4 launch plan", "container": {"title": "Q4 launch plan"},
            "history": {"lastUpdated": {"by": {"accountId": "frank", "displayName": "Frank"}}},
            "_links": {"webui": "/spaces/ENG/pages/98304/Q4+launch+plan?focusedCommentId=98400#comment-98400"}},
           "excerpt": "FYI @Alice, the dates moved &amp; nothing to do", "lastModified": "2026-10-12T10:00:00.000Z",
           "resultGlobalContainer": {"title": "Engineering"}},
          {"content": {"id": "98500", "type": "page", "title": "My notes",
            "history": {"lastUpdated": {"by": {"accountId": "alice-id", "displayName": "Alice"}}},
            "_links": {"webui": "/spaces/~alice/pages/98500"}},
           "excerpt": "@Alice to do", "lastModified": "2026-10-12T11:00:00.000Z", "resultGlobalContainer": {"title": "Alice"}}
        ]}
        """

    let cloudMe = #"{"accountId": "alice-id", "displayName": "Alice"}"#
    var cloudAuth: String { "Basic " + Data("alice@acme.com:t0ken".utf8).base64EncodedString() }

    func cloud(assigned: String? = nil, confluence: Bool = false) throws -> JiraPlugin {
        try JiraPlugin(
            config: config([
                "host": "acme.atlassian.net", "email": "alice@acme.com", "token": "t0ken",
                "confluence": confluence ? "true" : "",
            ]),
            http: JiraStub(
                myself: cloudMe, assigned: assigned ?? self.assigned, watched: watched(mentioning: "alice-id"),
                expectedAuthorization: cloudAuth))
    }

    /// An issue with a pull request still open waits on the review; the others stay to do. As Windows.
    @Test func anIssueWithAnOpenPullRequestWaits() async throws {
        var stub = JiraStub(
            myself: cloudMe, assigned: assigned, watched: watched(mentioning: "alice-id"),
            expectedAuthorization: cloudAuth)
        stub.withOpenChanges = #"{"issues": [{"id": "2", "key": "OPS-8", "fields": {"summary": "x"}}]}"#
        let plugin = try JiraPlugin(
            config: config(["host": "acme.atlassian.net", "email": "alice@acme.com", "token": "t0ken"]), http: stub)
        let items = try await plugin.fetch().items
        #expect(items.first { $0.context == "OPS-8" }?.bundle == .awaiting)
        #expect(items.first { $0.context == "OPS-7" }?.bundle == .tasks)
    }

    @Test func assignedIssuesAreTasksWithPriorityAndDueDate() async throws {
        let snapshot = try await cloud().fetch()
        #expect(snapshot.identity == "Alice")
        let tasks = snapshot.items.filter { $0.bundle == .tasks }
        #expect(tasks.map(\.context) == ["OPS-7", "OPS-8"])
        let urgent = try #require(tasks.first)
        #expect(urgent.priority == .urgent && urgent.needsAction)
        #expect(urgent.badges.map(\.id.rawValue).contains("overdue"))
        #expect(urgent.url?.absoluteString == "https://acme.atlassian.net/browse/OPS-7")
        #expect(tasks[1].priority == .low && !tasks[1].needsAction, "low priority is shown, not counted")
    }

    /// Recent comments from someone else that mention you; not old ones, not yours, not those without a mention.
    @Test func commentsThatMentionYouAreMentions() async throws {
        let mentions = try await cloud().fetch().items.filter { $0.bundle == .mentions }
        #expect(mentions.count == 1)
        let mention = try #require(mentions.first)
        #expect(mention.title == "Pager rota" && mention.context == "OPS-9")
        #expect(mention.preview == "@Alice can you take Friday?")
        #expect(mention.author?.name == "Erin")
        #expect(mention.url?.absoluteString == "https://acme.atlassian.net/browse/OPS-9?focusedCommentId=501")
    }

    /// Turned on: Confluence pages and comments that mention you, not those you changed last, then sorted by verb.
    @Test func confluenceMentionsAreMentionsWhenTurnedOn() async throws {
        #expect(try await cloud().fetch().items.filter { $0.id.contains("/confluence/") }.isEmpty, "off by default")
        let pages = try await cloud(confluence: true).fetch().items.filter { $0.id.contains("/confluence/") }
        #expect(pages.map(\.title) == ["Q4 launch plan", "Q4 launch plan"])
        let page = try #require(pages.first)
        #expect(page.bundle == .mentions && page.context == "Engineering")
        #expect(page.preview == "@Alice can you review this page?")
        #expect(page.author?.name == "Erin")
        #expect(page.author?.avatarURL?.absoluteString == "https://acme.atlassian.net/wiki/aa-avatar/erin")
        #expect(page.url?.absoluteString == "https://acme.atlassian.net/wiki/spaces/ENG/pages/98304/Q4+launch+plan")
        #expect(page.date == Date(iso8601: "2026-10-12T09:14:03.000Z"))
        let comment = pages[1]
        #expect(comment.preview == "FYI @Alice, the dates moved & nothing to do")
        #expect(
            comment.url?.absoluteString
                == "https://acme.atlassian.net/wiki/spaces/ENG/pages/98304/Q4+launch+plan?focusedCommentId=98400#comment-98400"
        )
        let classifier = VerbClassifier()
        #expect(classifier.classify(page).bundle == .reply)
        #expect(classifier.classify(comment).bundle == .read)
    }

    @Test func saysWhenIssuesAreCut() async throws {
        let more = assigned.replacingOccurrences(of: #""isLast": true"#, with: #""nextPageToken": "abc""#)
        #expect(try await cloud(assigned: more).fetch().remarks == [SourceSnapshot.truncated("Jira")])
        #expect(try await cloud().fetch().remarks.isEmpty)
    }

    /// Data Center: a personal access token as Bearer, `api/2`, wiki-markup comments with `[~name]`.
    @Test func dataCenterUsesABearerTokenAndWikiMentions() async throws {
        let watched = """
            {"issues": [{"id": "1", "key": "IT-3", "fields": {"summary": "VPN", "comment": {"comments": [
              {"id": "9", "author": {"name": "erin", "displayName": "Erin"}, "created": "\(recent)", "body": "[~alice] please check"}
            ]}}}], "total": 1}
            """
        let plugin = try JiraPlugin(
            config: config(["host": "jira.acme.io", "token": "pat"]),
            http: JiraStub(
                myself: #"{"name": "alice", "key": "JIRAUSER1", "displayName": "Alice"}"#,
                assigned: #"{"issues": [], "total": 0}"#, watched: watched, expectedAuthorization: "Bearer pat"))
        // Data Center's Confluence is another server: the option is for Cloud.
        let withConfluence = try JiraPlugin(
            config: config(["host": "jira.acme.io", "token": "pat", "confluence": "true"]),
            http: JiraStub(
                myself: #"{"name": "alice", "key": "JIRAUSER1", "displayName": "Alice"}"#,
                assigned: #"{"issues": [], "total": 0}"#, watched: watched, expectedAuthorization: "Bearer pat"))
        #expect(try await withConfluence.fetch().items.map(\.context) == ["IT-3"])
        let mentions = try await plugin.fetch().items
        #expect(mentions.map(\.context) == ["IT-3"])
        #expect(mentions.first?.preview == "@alice please check")
    }

    @Test func jiraCloudNeedsTheEmail() {
        #expect(throws: PluginError.missingField("email")) {
            try JiraPlugin(config: config(["host": "acme.atlassian.net", "token": "t"]), http: StubHTTP(routes: [:]))
        }
    }

    @Test func readsJiraTimestamps() {
        #expect(JiraPlugin.jiraDate("2026-10-12T09:14:03.123+0200") == Date(iso8601: "2026-10-12T07:14:03.123Z"))
        #expect(JiraPlugin.jiraDate("2026-10-12T09:14:03Z") != nil)
        #expect(JiraPlugin.jiraDate(nil) == nil)
    }
}

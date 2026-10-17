import Foundation
import RemoraCore

/// The persistence port: the inbox's files, written off the main thread by `writer`. Each service saves its own files
/// through it, right after each change. The demo saves nothing. Same on Windows (`persistence.rs`).
@MainActor
final class Persistence {
    let accounts: JSONStore<[Account]>
    let states: JSONStore<[String: ItemState]>
    let cache: JSONStore<[UUID: [InboxItem]]>
    let reminders: JSONStore<[InboxItem]>
    let preferences: JSONStore<Preferences>
    let brief: JSONStore<Brief?>
    let history: JSONStore<[SnoozeRecord]>
    let learning: JSONStore<[ActionRecord]>
    let reviewTimes: JSONStore<[ReviewTiming]>
    let bundleSummaries: JSONStore<[String: BundleSummary]>
    let assistantUsage: JSONStore<AssistantUsage>
    /// What agents asked through the MCP server.
    let agentReads: JSONStore<[AgentRead]>
    /// Saves the files off the main thread, each once per burst of changes.
    let writer = DiskWriter()
    private let isDemo: Bool

    init(folder: URL, demo: Bool) {
        isDemo = demo
        accounts = JSONStore("accounts", in: folder)
        states = JSONStore("states", in: folder)
        cache = JSONStore("cache", in: folder, backedUp: false)
        reminders = JSONStore("reminders", in: folder)
        preferences = JSONStore("preferences", in: folder)
        brief = JSONStore("brief", in: folder, backedUp: false)
        history = JSONStore("snooze-history", in: folder)
        learning = JSONStore("learning", in: folder)
        reviewTimes = JSONStore("review-times", in: folder)
        bundleSummaries = JSONStore("bundle-summaries", in: folder, backedUp: false)
        assistantUsage = JSONStore("assistant-usage", in: folder, backedUp: false)
        agentReads = JSONStore("agent-reads", in: folder, backedUp: false)
    }

    func save<Value: Sendable>(_ value: Value, to store: JSONStore<Value>) {
        guard !isDemo else { return }
        writer.save(value, to: store)
    }
}

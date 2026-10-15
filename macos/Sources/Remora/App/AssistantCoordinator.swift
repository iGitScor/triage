import Foundation
import Observation
import RemoraCore
import RemoraPlugins

/// The assistant coordinator: what the assistant wrote (the brief, each group's summary, the triage of the snoozed
/// pile), whether it is still fresh, what it is writing now and what failed. `InboxModel` decides who may be asked,
/// with what, and whether an answer may still be kept (`stillAnswers`). Same on Windows (`assistant.rs`).
@MainActor @Observable
final class AssistantCoordinator {
    private(set) var brief: Brief?
    private(set) var briefError: String?
    private(set) var isBriefing = false
    private(set) var bundleSummaries: [String: BundleSummary] = [:]
    private(set) var bundleSummaryErrors: [String: String] = [:]
    private(set) var summarizing: Set<String> = []
    private(set) var triageSuggestions: [TriageSuggestion] = []
    private(set) var triageError: String?
    private(set) var isTriaging = false
    /// What the assistant used, per day (AI-03).
    private(set) var usage: AssistantUsage
    /// Assistant requests a day, 0 for no limit: the user's or the organization's (`AIDailyLimit`).
    @ObservationIgnored var dailyLimit: () -> Int = { 0 }
    @ObservationIgnored private var triageKey: (ids: [String], at: Date)?
    @ObservationIgnored private let persistence: Persistence

    init(
        brief: Brief?, bundleSummaries: [String: BundleSummary], usage: AssistantUsage = AssistantUsage(),
        persistence: Persistence
    ) {
        self.brief = brief
        self.bundleSummaries = bundleSummaries
        self.usage = usage
        self.persistence = persistence
    }

    static let limitReached = L(
        "Today’s assistant limit is reached. It starts again tomorrow, or raise it in Settings.")

    /// One request to the assistant, within today's limit, counted with the tokens it reports whether it answers or
    /// fails (AI-03). Same on Windows (`Inbox::assistant`, `record_usage`).
    private func ask<Answer>(
        _ assistant: () throws -> any AssistantPlugin, _ request: (any AssistantPlugin) async throws -> Answer
    ) async throws -> Answer {
        guard usage.allows(limit: dailyLimit()) else { throw HTTPError.api(Self.limitReached) }
        let plugin = try assistant()
        let meter = AssistantMeter()
        defer { record(meter.tokens) }
        return try await AssistantMeter.$current.withValue(meter) { try await request(plugin) }
    }

    private func record(_ tokens: TokenUsage?) {
        usage.record(tokens)
        persistence.save(usage, to: persistence.assistantUsage)
    }

    func isBriefFresh(for items: [InboxItem], cacheMinutes: Int) -> Bool {
        brief?.isFresh(for: items, cacheMinutes: cacheMinutes, now: .now) ?? false
    }

    /// Writes a new brief from `items`, unless one is being written. `stillAnswers` says, once it is written,
    /// whether it may be kept (SEC-27).
    func makeBrief(
        from items: [InboxItem], assistant: () throws -> any AssistantPlugin, stillAnswers: () -> Bool
    ) async {
        guard !isBriefing else { return }
        isBriefing = true
        briefError = nil
        defer { isBriefing = false }
        do {
            let brief = try await ask(assistant) { try await $0.brief(items, now: .now) }.writtenFrom(items)
            guard stillAnswers() else { return }
            store(brief)
        } catch {
            briefError = error.localizedDescription
        }
    }

    /// Summarizes one bundle, reusing a fresh summary of the same items.
    func summarize(
        key: String, topic: String, items: [InboxItem], cacheMinutes: Int,
        assistant: () throws -> any AssistantPlugin, stillAnswers: () -> Bool
    ) async {
        guard !summarizing.contains(key) else { return }
        if bundleSummaries[key]?.isFresh(for: items, cacheMinutes: cacheMinutes, now: .now) == true { return }
        summarizing.insert(key)
        bundleSummaryErrors[key] = nil
        defer { summarizing.remove(key) }
        do {
            let text = try await ask(assistant) { try await $0.digest(items, topic: topic, now: .now) }
            guard stillAnswers() else { return }
            bundleSummaries[key] = BundleSummary(text: text, items: items)
            saveSummaries()
        } catch {
            bundleSummaryErrors[key] = error.localizedDescription
        }
    }

    /// Asks what to do with each snoozed item, reusing a fresh answer for the same pile (`ids`).
    func triage(
        ids: [String], items: () -> [SnoozedItem], cacheMinutes: Int,
        assistant: () throws -> any AssistantPlugin, stillAnswers: () -> Bool
    ) async {
        guard !isTriaging else { return }
        if let key = triageKey, key.ids == ids, key.at.addingTimeInterval(Double(cacheMinutes) * 60) > .now,
            !triageSuggestions.isEmpty
        {
            return
        }
        let items = items()
        isTriaging = true
        triageError = nil
        defer { isTriaging = false }
        do {
            let suggestions = try await ask(assistant) { try await $0.triage(items, now: .now) }
            guard stillAnswers() else { return }
            triageSuggestions = suggestions
            triageKey = (ids, .now)
        } catch {
            triageError = error.localizedDescription
        }
    }

    func store(_ brief: Brief?) {
        self.brief = brief
        persistence.save(brief, to: persistence.brief)
    }

    /// The suggestions applied leave the proposal; the others stay to be ticked.
    func applied(_ ids: Set<String>) {
        triageSuggestions.removeAll { ids.contains($0.id) }
    }

    func discardTriage() {
        triageSuggestions = []
        triageKey = nil
    }

    /// What may quote a source no longer allowed, or no longer has an assistant to keep it: the summaries and the
    /// triage go; the brief too when `brief` (no allowed assistant).
    func forget(brief: Bool) {
        guard self.brief != nil || !bundleSummaries.isEmpty || !triageSuggestions.isEmpty else { return }
        if brief { store(nil) }
        bundleSummaries = [:]
        saveSummaries()
        triageSuggestions = []
    }

    /// The assistant was disconnected: what it wrote goes.
    func disconnected() {
        store(nil)
        bundleSummaries = [:]
        saveSummaries()
    }

    /// Erase: nothing is left, and nothing is saved (the files go), what it used included.
    func reset() {
        brief = nil
        bundleSummaries = [:]
        triageSuggestions = []
        usage = AssistantUsage()
    }

    private func saveSummaries() {
        persistence.save(bundleSummaries, to: persistence.bundleSummaries)
    }
}

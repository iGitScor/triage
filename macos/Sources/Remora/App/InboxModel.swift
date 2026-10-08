import AppKit
import Observation
import RemoraCore
import RemoraPlugins

@MainActor @Observable
final class InboxModel {
    static let shared = InboxModel()

    private(set) var accounts: [Account]
    private(set) var itemsByAccount: [UUID: [InboxItem]]
    private(set) var reminders: [InboxItem]
    private(set) var states: [String: ItemState]
    private(set) var errors: [UUID: String] = [:]
    private(set) var isRefreshing = false
    private(set) var lastRefresh: Date?
    private(set) var now = Date.now
    private(set) var brief: Brief?
    private(set) var briefError: String?
    private(set) var isBriefing = false
    private(set) var bundleSummaries: [String: BundleSummary] = [:]
    private(set) var bundleSummaryErrors: [String: String] = [:]
    private(set) var summarizing: Set<String> = []
    private(set) var snoozeHistory: [SnoozeRecord] = []
    /// Patterns in the snoozed pile; recomputed when snoozes change, shown one at a time.
    private(set) var insights: [SnoozeInsight] = []
    private(set) var dismissedInsights: Set<String> = []
    private(set) var triageSuggestions: [TriageSuggestion] = []
    private(set) var triageError: String?
    private(set) var isTriaging = false
    /// Item ID → items from other tools about the same thing. Computed in the background after each refresh.
    private(set) var links: [String: [String]] = [:]
    /// Learned from your actions on this Mac; silent until it has enough of them.
    private(set) var ranker = PersonalRanker(records: [])
    @ObservationIgnored private var actionRecords: [ActionRecord] = []
    var query = ""
    var preferences: Preferences {
        didSet {
            preferencesStore.save(preferences)
            if oldValue.refreshMinutes != preferences.refreshMinutes { startPolling() }
        }
    }

    @ObservationIgnored private let http: HTTPClient = URLSessionHTTPClient()
    /// Keywords, then on-device sentence embeddings. Slow on first sight of a message, so it runs off the main thread.
    @ObservationIgnored private let classifier = VerbClassifier([KeywordIntentClassifier(), EmbeddingIntentClassifier()])
    @ObservationIgnored private let accountsStore = JSONStore<[Account]>("accounts")
    @ObservationIgnored private let statesStore = JSONStore<[String: ItemState]>("states")
    @ObservationIgnored private let cacheStore = JSONStore<[UUID: [InboxItem]]>("cache")
    @ObservationIgnored private let remindersStore = JSONStore<[InboxItem]>("reminders")
    @ObservationIgnored private let preferencesStore = JSONStore<Preferences>("preferences")
    @ObservationIgnored private let briefStore = JSONStore<Brief?>("brief")
    @ObservationIgnored private let historyStore = JSONStore<[SnoozeRecord]>("snooze-history")
    @ObservationIgnored private let learningStore = JSONStore<[ActionRecord]>("learning")
    @ObservationIgnored let advisor = SnoozeAdvisor(similarity: [KeywordSimilarity(), EmbeddingSimilarity()])
    @ObservationIgnored private var triageKey: (ids: [String], at: Date)?
    @ObservationIgnored private let linkFinder = LinkFinder(
        similarity: [KeywordSimilarity(), RareTokenSimilarity(), EmbeddingSimilarity(threshold: 0.9)]
    )
    @ObservationIgnored private let bundleSummariesStore = JSONStore<[String: BundleSummary]>("bundle-summaries")
    @ObservationIgnored private var polling: Task<Void, Never>?
    @ObservationIgnored private var ticking: Task<Void, Never>?
    @ObservationIgnored private var syncedAccounts: Set<UUID> = []
    @ObservationIgnored private let isDemo = CommandLine.arguments.contains("--demo")

    private init() {
        preferences = preferencesStore.load() ?? Preferences()
        if isDemo {
            accounts = []
            let demoItems = DemoData.items().mapValues { $0.map(VerbClassifier().classify) }
            itemsByAccount = demoItems
            brief = DemoData.brief
            reminders = []
            let demo = DemoData.snoozes(items: demoItems.values.flatMap { $0 })
            states = demo.states
            snoozeHistory = demo.history
        } else {
            accounts = accountsStore.load() ?? []
            itemsByAccount = (cacheStore.load() ?? [:]).mapValues { $0.map(VerbClassifier().classify) }
            reminders = remindersStore.load() ?? []
            states = statesStore.load() ?? [:]
            brief = briefStore.load() ?? nil
            bundleSummaries = bundleSummariesStore.load() ?? [:]
            snoozeHistory = historyStore.load() ?? []
            actionRecords = learningStore.load() ?? []
            ranker = PersonalRanker(records: actionRecords)
        }
    }

    // MARK: Reading

    var allItems: [InboxItem] { itemsByAccount.values.flatMap { $0 } + reminders }

    var layout: InboxLayout {
        let ranker = ranker
        return InboxAssembler(wakeOnActivity: preferences.wakeOnActivity, personal: { ranker.score($0) })
            .layout(items: itemsWithLinkedPriority, states: states, now: now, query: query)
    }

    /// A PR linked to an urgent ticket is urgent too.
    private var itemsWithLinkedPriority: [InboxItem] {
        let items = allItems
        guard !links.isEmpty else { return items }
        let priorities = Dictionary(items.map { ($0.id, $0.priority) }, uniquingKeysWith: { first, _ in first })
        return items.map { item in
            var item = item
            for linked in links[item.id] ?? [] {
                item.priority = Priority.max(item.priority, priorities[linked] ?? nil)
            }
            return item
        }
    }

    func linkedItems(_ item: InboxItem) -> [InboxItem] {
        (links[item.id] ?? []).compactMap { self.item($0) }
    }

    private func recomputeLinks() {
        let items = allItems
        let finder = linkFinder
        Task {
            let links = await Task.detached { finder.links(items) }.value
            self.links = links
        }
    }

    func state(of item: InboxItem) -> ItemState { states[item.id] ?? ItemState() }

    func account(_ id: UUID) -> Account? { accounts.first { $0.id == id } }

    func rename(_ account: Account, to name: String) {
        guard let index = accounts.firstIndex(where: { $0.id == account.id }) else { return }
        accounts[index].name = name.trimmingCharacters(in: .whitespaces).nilIfEmpty
        persist(accountsStore, accounts)
    }

    /// The account's name, shown on items only when several accounts of the same plugin are connected.
    func accountLabel(for item: InboxItem) -> String? {
        guard accounts.filter({ $0.pluginID == item.pluginID }).count > 1, let account = account(item.accountID) else { return nil }
        return account.name ?? account.identity
    }

    var sourceAccounts: [Account] { accounts.filter { !PluginRegistry.isAssistant($0.pluginID) } }

    /// The connected assistant, only when the privacy policy allows it.
    var assistantAccount: Account? {
        accounts.first { account in
            PluginRegistry.isAssistant(account.pluginID)
                && PluginRegistry.manifest(account.pluginID).map(policy.allows) == true
        }
    }

    // MARK: Privacy

    var policy: CompliancePolicy { ManagedPolicy.policy(user: preferences) }

    /// The only way to build a plugin: it must be allowed, and it can only reach its declared hosts.
    private func sourcePlugin(_ account: Account, secrets: [String: String]) throws -> any SourcePlugin {
        try PluginRegistry.make(account, secrets: secrets, http: guardedClient(for: account))
    }

    private func assistantPlugin(_ account: Account, secrets: [String: String]) throws -> any AssistantPlugin {
        try PluginRegistry.makeAssistant(account, secrets: secrets, http: guardedClient(for: account))
    }

    private func guardedClient(for account: Account) throws -> HTTPClient {
        guard let manifest = PluginRegistry.manifest(account.pluginID) else { throw PluginError.unknownPlugin(account.pluginID) }
        if let refusal = policy.refusal(for: manifest) { throw EgressError.blockedPlugin(refusal) }
        return GuardedHTTPClient(http, allowing: allowedHosts(for: account, manifest: manifest))
    }

    func allowedHosts(for account: Account, manifest: PluginManifest) -> [String] {
        let own = account.settings["host"].flatMap { URL(string: $0.contains("://") ? $0 : "https://" + $0)?.host }
        return manifest.egress.hosts + [own].compactMap { $0 }
    }

    /// Avatars load only from the hosts of allowed, connected tools.
    func allowsImage(_ url: URL?) -> Bool {
        guard policy.allowRemoteImages, let host = url?.host?.lowercased() else { return false }
        return sourceAccounts.contains { account in
            guard let manifest = PluginRegistry.manifest(account.pluginID), policy.allows(manifest) else { return false }
            return GuardedHTTPClient.matches(host, allowedHosts(for: account, manifest: manifest))
        }
    }

    // MARK: Assistant

    /// Writes a new brief, unless the current one is still fresh (Settings → General → Assistant).
    func makeBrief() async {
        guard preferences.wholeInboxBrief, let account = assistantAccount, !isBriefing, !isBriefFresh else { return }
        isBriefing = true
        briefError = nil
        defer { isBriefing = false }
        let layout = layout
        let items = layout.pinned + layout.groups.flatMap(\.items)
        do {
            let assistant = try assistantPlugin(account, secrets: Keychain.secrets(for: account.id))
            store(try await assistant.brief(items, now: .now))
        } catch {
            briefError = error.localizedDescription
        }
    }

    /// Summarizes one bundle with the assistant's lighter model, reusing a fresh summary of the same items.
    func summarize(key: String, topic: String, items: [InboxItem]) async {
        guard let account = assistantAccount, !summarizing.contains(key) else { return }
        if bundleSummaries[key]?.isFresh(for: items, cacheMinutes: preferences.briefCacheMinutes, now: .now) == true { return }
        summarizing.insert(key)
        bundleSummaryErrors[key] = nil
        defer { summarizing.remove(key) }
        do {
            let assistant = try assistantPlugin(account, secrets: Keychain.secrets(for: account.id))
            let text = try await assistant.digest(items, topic: topic, now: .now)
            bundleSummaries[key] = BundleSummary(text: text, items: items)
            persist(bundleSummariesStore, bundleSummaries)
        } catch {
            bundleSummaryErrors[key] = error.localizedDescription
        }
    }

    var isBriefFresh: Bool {
        brief?.isFresh(cacheMinutes: preferences.briefCacheMinutes, now: now) ?? false
    }

    private func store(_ brief: Brief?) {
        self.brief = brief
        persist(briefStore, brief)
    }

    func item(_ id: String) -> InboxItem? { allItems.first { $0.id == id } }

    // MARK: Lifecycle

    func start() {
        refreshInsights()
        recomputeLinks()
        if !isDemo { startPolling() }
        ticking = Task { [weak self] in
            while !Task.isCancelled {
                self?.tick()
                try? await Task.sleep(for: .seconds(20))
            }
        }
    }

    private func startPolling() {
        polling?.cancel()
        let interval = Double(max(1, preferences.refreshMinutes) * 60)
        polling = Task { [weak self] in
            while !Task.isCancelled {
                await self?.refresh()
                try? await Task.sleep(for: .seconds(interval))
            }
        }
    }

    func refreshIfStale() {
        guard (lastRefresh ?? .distantPast).timeIntervalSinceNow < -60 else { return }
        Task { await refresh() }
    }

    func refresh() async {
        guard !isRefreshing else { return }
        isRefreshing = true
        defer { isRefreshing = false }

        let classifier = classifier
        let jobs = sourceAccounts.map { account in
            (account.id, Result { try sourcePlugin(account, secrets: Keychain.secrets(for: account.id)) })
        }
        await withTaskGroup(of: (UUID, Result<SourceSnapshot, Error>).self) { group in
            for (id, made) in jobs {
                group.addTask {
                    do {
                        let plugin = try made.get()
                        var snapshot = try await plugin.fetch()
                        snapshot.items = snapshot.items.map(classifier.classify)
                        return (id, .success(snapshot))
                    } catch {
                        return (id, .failure(error))
                    }
                }
            }
            for await (id, result) in group {
                switch result {
                case .success(let snapshot): apply(snapshot, to: id)
                case .failure(let error): errors[id] = error.localizedDescription
                }
            }
        }
        lastRefresh = .now
        persist(cacheStore, itemsByAccount)
        pruneStates()
        refreshInsights()
        recomputeLinks()
    }

    private func apply(_ snapshot: SourceSnapshot, to accountID: UUID) {
        let previous = syncedAccounts.contains(accountID) ? itemsByAccount[accountID] : nil
        syncedAccounts.insert(accountID)
        errors[accountID] = nil
        itemsByAccount[accountID] = snapshot.items

        if let index = accounts.firstIndex(where: { $0.id == accountID }), accounts[index].identity != snapshot.identity {
            accounts[index].identity = snapshot.identity
            persist(accountsStore, accounts)
        }

        for notice in ChangeDetector.notices(previous: previous, current: snapshot.items) {
            let wanted = notice.kind == .arrival ? preferences.notifyArrivals : preferences.notifyStatusChanges
            if wanted { Notifier.shared.post(notice) }
        }
        wakeSnoozedItems(touchedIn: snapshot.items)
    }

    private func wakeSnoozedItems(touchedIn items: [InboxItem]) {
        guard preferences.wakeOnActivity else { return }
        for item in items {
            guard let snooze = states[item.id]?.snooze, snooze.mode == .hide, snooze.fingerprint != item.fingerprint else { continue }
            update(item) { $0.snooze = nil }
            Notifier.shared.cancel(item.id)
        }
    }

    /// Wakes up items whose snooze is over. The notification itself was scheduled with the system.
    private func tick() {
        now = .now
        var changed = false
        for (id, state) in states {
            guard let snooze = state.snooze, snooze.until <= now else { continue }
            states[id]?.snooze = nil
            states[id]?.remindedAt = now
            states[id]?.done = nil
            changed = true
        }
        if changed {
            persist(statesStore, states)
            refreshInsights()
        }
    }

    private func pruneStates() {
        let known = Set(allItems.map(\.id))
        let before = states.count
        states = states.filter { known.contains($0.key) || $0.value.snooze != nil }
        if states.count != before { persist(statesStore, states) }
    }

    // MARK: Actions

    /// The desktop app when it's installed and handles the link (slack://, linear://), else the browser.
    /// `inBrowser` forces the web page (⌥-click), e.g. for the exact Slack message.
    func open(_ item: InboxItem, inBrowser: Bool = false) {
        if !inBrowser, preferences.openInApps, let app = item.appURL,
           NSWorkspace.shared.urlForApplication(toOpen: app) != nil, NSWorkspace.shared.open(app) {
            // Opened in the app.
        } else if let url = item.url {
            NSWorkspace.shared.open(url)
        }
        update(item) { $0.remindedAt = nil }
        learnHandled(item)
    }

    // MARK: Waiting

    @ObservationIgnored let waitingAssistant = WaitingAssistant()

    /// Reviewer suggestions or a nudge for one of your MRs that's waiting.
    func waitingHelp(_ item: InboxItem) -> WaitingAssistant.Help? {
        waitingAssistant.help(for: item, among: allItems, me: account(item.accountID)?.identity, now: now)
    }

    /// Why an item ranks higher than its priority alone would put it.
    func rankingReason(_ item: InboxItem) -> String? { ranker.reason(item) }

    /// Opened or finished within a day of its last activity: something you handle quickly.
    private func learnHandled(_ item: InboxItem) {
        guard item.bundle != .reminders, Date.now.timeIntervalSince(item.date) < 86_400 else { return }
        learn(ActionRecord(item: item, outcome: .quick))
    }

    private func learn(_ record: ActionRecord) {
        actionRecords = Array((actionRecords + [record]).suffix(1_000))
        ranker = PersonalRanker(records: actionRecords)
        persist(learningStore, actionRecords)
    }

    func toggleDone(_ item: InboxItem) {
        if item.bundle == .reminders, state(of: item).done == nil {
            removeReminder(item)
            return
        }
        let markingDone = state(of: item).done == nil
        update(item) { state in
            state.done = markingDone ? .init(at: .now, fingerprint: item.fingerprint) : nil
            state.remindedAt = nil
            state.snooze = nil
        }
        Notifier.shared.cancel(item.id)
        if markingDone { learnHandled(item) }
        if markingDone, let index = snoozeHistory.lastIndex(where: { $0.itemID == item.id && $0.doneAt == nil }) {
            snoozeHistory[index].doneAt = .now
            persist(historyStore, snoozeHistory)
        }
        refreshInsights()
    }

    func clear(_ items: [InboxItem]) {
        for item in items {
            update(item) { $0.done?.clearedAt = .now }
        }
    }

    func sweep(_ items: [InboxItem]) {
        items.forEach(toggleDone)
    }

    func togglePin(_ item: InboxItem) {
        update(item) { $0.pinned.toggle() }
    }

    func snooze(
        _ item: InboxItem,
        until date: Date,
        mode: Snooze.Mode,
        note: String?,
        reason: SnoozeReason? = nil,
        untilNews: Bool = false
    ) {
        let note = note?.trimmingCharacters(in: .whitespaces).nilIfEmpty
        update(item) { state in
            state.snooze = Snooze(
                until: date, mode: mode, note: note, fingerprint: item.fingerprint,
                reason: reason, untilNews: untilNews ? true : nil
            )
            state.done = nil
            state.remindedAt = nil
        }
        if item.bundle != .reminders {
            learn(ActionRecord(item: item, outcome: .deferred))
            snoozeHistory.append(SnoozeRecord(item: item, reason: reason, at: .now, until: date))
            snoozeHistory = Array(snoozeHistory.suffix(500))
            persist(historyStore, snoozeHistory)
        }
        scheduleReturn(of: item, at: date, note: note)
        refreshInsights()
    }

    func snoozeMany(_ items: [InboxItem], until date: Date, reason: SnoozeReason?, untilNews: Bool = false) {
        for item in items {
            snooze(item, until: date, mode: .hide, note: nil, reason: reason, untilNews: untilNews)
        }
    }

    /// Moves an existing snooze without counting a new one in the history.
    func reschedule(_ item: InboxItem, to date: Date) {
        guard var snooze = state(of: item).snooze else {
            snooze(item, until: date, mode: .hide, note: nil)
            return
        }
        snooze.until = date
        update(item) { $0.snooze = snooze }
        scheduleReturn(of: item, at: date, note: snooze.note)
        refreshInsights()
    }

    private func scheduleReturn(of item: InboxItem, at date: Date, note: String?) {
        Notifier.shared.cancel(item.id)
        Notifier.shared.schedule(Notice(
            kind: .reminder,
            itemID: item.id,
            title: L(item.bundle == .reminders ? "Reminder" : "Back in your inbox"),
            subtitle: item.context,
            body: [item.title, note].compactMap { $0 }.joined(separator: "\n"),
            url: item.url
        ), at: date)
    }

    func unsnooze(_ item: InboxItem) {
        update(item) { $0.snooze = nil }
        Notifier.shared.cancel(item.id)
        refreshInsights()
    }

    // MARK: Smart snooze

    /// Times this item was snoozed in the last 30 days.
    func snoozeCount(_ item: InboxItem) -> Int {
        let since = Date.now.addingTimeInterval(-30 * 86_400)
        return snoozeHistory.filter { $0.itemID == item.id && $0.at > since }.count
    }

    func suggestedReturn(for reason: SnoozeReason) -> Date {
        advisor.suggestedReturn(for: reason, history: snoozeHistory)
    }

    func usualReason(for item: InboxItem) -> SnoozeReason? {
        advisor.usualReason(for: item, history: snoozeHistory)
    }

    func usualReturn(for item: InboxItem) -> Date? {
        advisor.usualReturn(for: item, history: snoozeHistory)
    }

    var visibleInsights: [SnoozeInsight] {
        insights.filter { !dismissedInsights.contains($0.id) }
    }

    func refreshInsights() {
        insights = advisor.insights(snoozed: layout.snoozed, states: states, history: snoozeHistory, now: .now)
    }

    func dismiss(_ insight: SnoozeInsight) {
        dismissedInsights.insert(insight.id)
    }

    /// Spreads a pile-up 30 minutes apart from its slot.
    func spread(_ items: [InboxItem], from start: Date) {
        for (id, date) in advisor.spread(items, from: start) {
            if let item = item(id) { reschedule(item, to: date) }
        }
    }

    /// Brings every item back at the earliest of their return times.
    func alignReturns(_ items: [InboxItem]) {
        guard let earliest = items.compactMap({ state(of: $0).snooze?.until }).min() else { return }
        items.forEach { reschedule($0, to: earliest) }
    }

    /// Asks the assistant what to do with each snoozed item, reusing a fresh answer for the same pile.
    func triage() async {
        guard let account = assistantAccount, !isTriaging else { return }
        let snoozed = layout.snoozed
        let ids = snoozed.map(\.id).sorted()
        if let key = triageKey, key.ids == ids,
           key.at.addingTimeInterval(Double(preferences.briefCacheMinutes) * 60) > .now, !triageSuggestions.isEmpty {
            return
        }
        let items = snoozed.compactMap { item in
            state(of: item).snooze.map { SnoozedItem(item: item, snooze: $0, times: snoozeCount(item)) }
        }
        isTriaging = true
        triageError = nil
        defer { isTriaging = false }
        do {
            let assistant = try assistantPlugin(account, secrets: Keychain.secrets(for: account.id))
            triageSuggestions = try await assistant.triage(items, now: .now)
            triageKey = (ids, .now)
        } catch {
            triageError = error.localizedDescription
        }
    }

    func apply(_ suggestions: [TriageSuggestion]) {
        for suggestion in suggestions {
            guard let item = item(suggestion.id) else { continue }
            switch suggestion.action {
            case .keep: break
            case .reschedule: if let until = suggestion.until { reschedule(item, to: until) }
            case .done: toggleDone(item)
            case .now:
                unsnooze(item)
                open(item)
            }
        }
        let applied = Set(suggestions.map(\.id))
        triageSuggestions.removeAll { applied.contains($0.id) }
    }

    func discardTriage() {
        triageSuggestions = []
        triageKey = nil
    }

    /// A notification was clicked: open its item like a click in the inbox.
    @discardableResult
    func openFromNotification(_ itemID: String) -> Bool {
        guard let item = item(itemID) else { return false }
        open(item)
        return true
    }

    /// A button pressed on a notification.
    func perform(_ action: NotificationAction, onItem itemID: String) {
        guard let item = item(itemID) else { return }
        switch action {
        case .done where state(of: item).done == nil:
            toggleDone(item)
        case .done:
            break
        case .snoozeTenMinutes:
            snooze(item, until: .now.addingTimeInterval(10 * 60), mode: .hide, note: nil)
        case .snoozeHour:
            snooze(item, until: .now.addingTimeInterval(3_600), mode: .hide, note: nil)
        case .snoozeTomorrow:
            snooze(item, until: SnoozeClock().tomorrowMorning(), mode: .hide, note: nil)
        }
    }

    func addReminder(_ title: String, at date: Date) {
        let item = InboxItem(
            id: "reminder/\(UUID().uuidString)",
            accountID: UUID(),
            pluginID: "reminders",
            bundle: .reminders,
            title: title,
            context: "Reminder",
            date: .now
        )
        reminders.append(item)
        persist(remindersStore, reminders)
        snooze(item, until: date, mode: .hide, note: nil)
    }

    private func removeReminder(_ item: InboxItem) {
        reminders.removeAll { $0.id == item.id }
        states[item.id] = nil
        persist(remindersStore, reminders)
        persist(statesStore, states)
        Notifier.shared.cancel(item.id)
    }

    private func update(_ item: InboxItem, _ change: (inout ItemState) -> Void) {
        var state = states[item.id] ?? ItemState()
        change(&state)
        states[item.id] = state.isEmpty ? nil : state
        persist(statesStore, states)
    }

    // MARK: Accounts

    func connect(pluginID: String, name: String?, settings: [String: String], secrets: [String: String]) async throws {
        let name = name?.trimmingCharacters(in: .whitespaces).nilIfEmpty
        let account = Account(pluginID: pluginID, name: name, settings: settings)
        if PluginRegistry.isAssistant(pluginID) {
            let assistant = try assistantPlugin(account, secrets: secrets)
            store(try await assistant.brief(layout.groups.flatMap(\.items), now: .now))
            Keychain.save(secrets, for: account.id)
            accounts.removeAll { PluginRegistry.isAssistant($0.pluginID) }
            accounts.append(account)
            persist(accountsStore, accounts)
            return
        }
        let plugin = try sourcePlugin(account, secrets: secrets)
        var snapshot = try await plugin.fetch()
        let classifier = classifier
        snapshot.items = await Task.detached { snapshot.items.map(classifier.classify) }.value
        Keychain.save(secrets, for: account.id)
        accounts.append(account)
        persist(accountsStore, accounts)
        apply(snapshot, to: account.id)
        persist(cacheStore, itemsByAccount)
    }

    /// Removes everything Remora stored on this Mac: accounts, tokens, cache, history, settings.
    func eraseLocalData() {
        accounts.forEach { Keychain.delete($0.id) }
        accounts = []
        itemsByAccount = [:]
        reminders = []
        states = [:]
        errors = [:]
        brief = nil
        bundleSummaries = [:]
        snoozeHistory = []
        actionRecords = []
        ranker = PersonalRanker(records: [])
        links = [:]
        triageSuggestions = []
        insights = []
        syncedAccounts = []
        preferences = Preferences()
        if let files = try? FileManager.default.contentsOfDirectory(at: AppFolder.url, includingPropertiesForKeys: nil) {
            files.forEach { try? FileManager.default.removeItem(at: $0) }
        }
    }

    func disconnect(_ account: Account) {
        accounts.removeAll { $0.id == account.id }
        if PluginRegistry.isAssistant(account.pluginID) {
            store(nil)
            bundleSummaries = [:]
            persist(bundleSummariesStore, bundleSummaries)
        }
        itemsByAccount[account.id] = nil
        errors[account.id] = nil
        syncedAccounts.remove(account.id)
        Keychain.delete(account.id)
        persist(accountsStore, accounts)
        persist(cacheStore, itemsByAccount)
        pruneStates()
    }
}

extension InboxModel {
    fileprivate func persist<Value>(_ store: JSONStore<Value>, _ value: Value) {
        if !isDemo { store.save(value) }
    }
}

extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
}

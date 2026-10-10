import AppKit
import Observation
import RemoraCore
import RemoraPlugins

@MainActor @Observable
final class InboxModel {
    static let shared = InboxModel(environment: .live)

    private(set) var accounts: [Account]
    private(set) var itemsByAccount: [UUID: [InboxItem]] { didSet { layoutChanged() } }
    private(set) var reminders: [InboxItem] { didSet { layoutChanged() } }
    private(set) var states: [String: ItemState] { didSet { layoutChanged() } }
    /// Each failing account's last failure, by kind.
    private(set) var errors: [UUID: SourceFailure] = [:]
    struct StorageIssue: Equatable, Identifiable {
        var file: String
        /// Read and kept aside at launch, rather than failing to save.
        var corrupt: Bool
        var message: String
        var id: String { file + (corrupt ? "/read" : "/write") }
    }

    /// Files that couldn't be read (kept aside) or saved, for Settings → Privacy.
    private(set) var storageIssues: [StorageIssue] = []
    /// No network: refreshes wait for it, and the footer says so once instead of every source failing.
    private(set) var isOffline = false
    /// What the last fetch of each account said besides its items: a list cut at its limit, a missing permission.
    private(set) var remarks: [UUID: [String]] = [:]
    private(set) var isRefreshing = false
    private(set) var lastRefresh: Date?
    /// When the passing of time changed the layout: it moves only when an item expires or a new day starts
    /// (what's due today). Snoozes ending change `states` instead. The layout itself is computed at the real time.
    private(set) var now = Date.now { didSet { layoutChanged() } }
    private(set) var brief: Brief?
    private(set) var briefError: String?
    private(set) var isBriefing = false
    private(set) var bundleSummaries: [String: BundleSummary] = [:]
    private(set) var bundleSummaryErrors: [String: String] = [:]
    private(set) var summarizing: Set<String> = []
    private(set) var snoozeHistory: [SnoozeRecord] = [] { didSet { cachedSnoozeCounts = nil } }
    /// Patterns in the snoozed pile; recomputed when snoozes change, shown one at a time.
    private(set) var insights: [SnoozeInsight] = []
    private(set) var dismissedInsights: Set<String> = []
    private(set) var triageSuggestions: [TriageSuggestion] = []
    private(set) var triageError: String?
    private(set) var isTriaging = false
    /// Item ID → items from other tools about the same thing. Computed in the background after each refresh.
    private(set) var links: [String: [String]] = [:] { didSet { layoutChanged() } }
    /// Learned from your actions on this Mac; silent until it has enough of them.
    private(set) var ranker = PersonalRanker(records: []) { didSet { layoutChanged() } }
    /// Reviews you timed (Start, then Done): how long they really took.
    private(set) var reviewTimings: [ReviewTiming] = []
    /// How your reviews compare with the estimate; 1 until five were timed.
    var reviewPace: Double { ReviewPace.factor(reviewTimings) }
    @ObservationIgnored private var actionRecords: [ActionRecord] = []
    var query = ""
    /// Bumped each time the menu bar popover opens, so the inbox can pick its starting tab.
    var popoverOpenings = 0
    var preferences: Preferences {
        didSet {
            persist(preferencesStore, preferences)
            if oldValue.wakeOnActivity != preferences.wakeOnActivity { layoutChanged() }
            Myna.textScale = preferences.textSize.scale
            if oldValue.allowedPlugins != preferences.allowedPlugins
                || oldValue.allowExternalAI != preferences.allowExternalAI
            {
                enforcePolicy()
            }
            if ManagedPolicy.effective(oldValue, managed: environment.managed).refreshMinutes
                != effectivePreferences.refreshMinutes
            {
                startPolling()
            }
            if oldValue.checkForUpdates != preferences.checkForUpdates { scheduleUpdates() }
        }
    }

    @ObservationIgnored private let environment: AppEnvironment
    let updater: Updater
    @ObservationIgnored private let http: HTTPClient
    /// Keywords, then on-device sentence embeddings. Slow on first sight of a message, so it runs off the main thread.
    @ObservationIgnored private let classifier = VerbClassifier([
        KeywordIntentClassifier(), EmbeddingIntentClassifier(),
    ])
    @ObservationIgnored private let accountsStore: JSONStore<[Account]>
    @ObservationIgnored private let statesStore: JSONStore<[String: ItemState]>
    @ObservationIgnored private let cacheStore: JSONStore<[UUID: [InboxItem]]>
    @ObservationIgnored private let remindersStore: JSONStore<[InboxItem]>
    @ObservationIgnored private let preferencesStore: JSONStore<Preferences>
    @ObservationIgnored private let briefStore: JSONStore<Brief?>
    @ObservationIgnored private let historyStore: JSONStore<[SnoozeRecord]>
    @ObservationIgnored private let learningStore: JSONStore<[ActionRecord]>
    @ObservationIgnored private let reviewTimesStore: JSONStore<[ReviewTiming]>
    @ObservationIgnored let advisor = SnoozeAdvisor(similarity: [KeywordSimilarity(), EmbeddingSimilarity()])
    @ObservationIgnored private var triageKey: (ids: [String], at: Date)?
    @ObservationIgnored private let linkFinder = LinkFinder(
        similarity: [KeywordSimilarity(), RareTokenSimilarity(), EmbeddingSimilarity(threshold: 0.9)]
    )
    @ObservationIgnored private let bundleSummariesStore: JSONStore<[String: BundleSummary]>
    @ObservationIgnored private var polling: Task<Void, Never>?
    @ObservationIgnored private var ticking: Task<Void, Never>?
    @ObservationIgnored private var syncedAccounts: Set<UUID> = []
    @ObservationIgnored private let isDemo: Bool

    init(environment: AppEnvironment) {
        self.environment = environment
        http = environment.http
        updater = Updater(system: environment.updates)
        isDemo = environment.demo
        let folder = environment.folder
        accountsStore = JSONStore("accounts", in: folder)
        statesStore = JSONStore("states", in: folder)
        cacheStore = JSONStore("cache", in: folder, backedUp: false)
        remindersStore = JSONStore("reminders", in: folder)
        preferencesStore = JSONStore("preferences", in: folder)
        briefStore = JSONStore("brief", in: folder, backedUp: false)
        historyStore = JSONStore("snooze-history", in: folder)
        learningStore = JSONStore("learning", in: folder)
        reviewTimesStore = JSONStore("review-times", in: folder)
        bundleSummariesStore = JSONStore("bundle-summaries", in: folder, backedUp: false)
        // A file that can't be read is kept aside and named in Settings → Privacy, never overwritten.
        var issues: [StorageIssue] = []
        func read<Value>(_ store: JSONStore<Value>) -> Value? {
            do {
                return try store.read()
            } catch {
                issues.append(
                    StorageIssue(file: store.url.lastPathComponent, corrupt: true, message: error.localizedDescription))
                return nil
            }
        }
        let preferences = read(preferencesStore) ?? Preferences()
        self.preferences = preferences
        Myna.textScale = preferences.textSize.scale
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
            accounts = read(accountsStore) ?? []
            // Fetched again at the next refresh: a broken cache or brief is just started again.
            itemsByAccount = (cacheStore.load() ?? [:]).mapValues { $0.map(VerbClassifier().classify) }
            reminders = read(remindersStore) ?? []
            states = read(statesStore) ?? [:]
            brief = briefStore.load() ?? nil
            bundleSummaries = bundleSummariesStore.load() ?? [:]
            snoozeHistory = read(historyStore) ?? []
            actionRecords = read(learningStore) ?? []
            reviewTimings = read(reviewTimesStore) ?? []
            ranker = PersonalRanker(records: actionRecords)
            environment.removeURLCaches()
        }
        storageIssues = issues
    }

    // MARK: Reading

    var allItems: [InboxItem] { itemsByAccount.values.flatMap { $0 } + reminders }

    /// The whole inbox: what the menu bar counts and what the brief, triage and insights read. A search never
    /// changes it.
    var layout: InboxLayout { layout(query: "") }

    /// What the inbox shows: the same, narrowed by the search field.
    var visibleLayout: InboxLayout { layout(query: query) }

    /// The layout is computed once per change of what it reads, not on every read. Readers observe
    /// `layoutVersion`, so they still update when it changes.
    private(set) var layoutVersion = 0
    @ObservationIgnored private var cachedLayouts: (version: Int, byQuery: [String: InboxLayout]) = (-1, [:])

    private func layoutChanged() {
        layoutVersion &+= 1
    }

    private func layout(query: String) -> InboxLayout {
        let version = layoutVersion
        if cachedLayouts.version != version { cachedLayouts = (version, [:]) }
        if let cached = cachedLayouts.byQuery[query] { return cached }
        let ranker = ranker
        // The real time, not `now`: items fetched since may have expired already.
        let layout = InboxAssembler(wakeOnActivity: preferences.wakeOnActivity, personal: { ranker.score($0) })
            .layout(items: itemsWithLinkedPriority, states: states, now: .now, query: query)
        // The whole inbox and the current search: an earlier search isn't asked for again.
        cachedLayouts.byQuery = cachedLayouts.byQuery.filter { $0.key.isEmpty }
        cachedLayouts.byQuery[query] = layout
        return layout
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
        guard accounts.filter({ $0.pluginID == item.pluginID }).count > 1, let account = account(item.accountID) else {
            return nil
        }
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

    var policy: CompliancePolicy { ManagedPolicy.policy(user: preferences, managed: environment.managed) }

    /// What Remora acts on: your preferences with the organization's settings in place.
    var effectivePreferences: Preferences { ManagedPolicy.effective(preferences, managed: environment.managed) }

    /// What the assistant may read and which models it may use.
    var assistantPolicy: AssistantPolicy {
        ManagedPolicy.assistantPolicy(user: preferences, managed: environment.managed)
    }

    /// What the privacy policy refuses leaves the screen and the disk at once, not at the next successful
    /// refresh: a refused tool's cached items, its notifications and the AI summaries that may quote them. Its account
    /// stays, saying why and how to delete its token. Without an allowed assistant, the brief, summaries and triage
    /// suggestions go. Runs at launch, at each refresh (a new MDM profile) and when Privacy settings change.
    func enforcePolicy() {
        guard !isDemo else { return }
        let policy = policy
        var dropped = false
        for account in accounts {
            guard let manifest = PluginRegistry.manifest(account.pluginID), let refusal = policy.refusal(for: manifest)
            else { continue }
            errors[account.id] = SourceFailure(
                kind: .other, message: L("%@ Remove the account to delete its token.", refusal))
            if itemsByAccount[account.id] != nil {
                itemsByAccount[account.id] = nil
                syncedAccounts.remove(account.id)
                environment.notifications.remove(itemsWithPrefix: "\(account.id.uuidString)/")
                dropped = true
            }
        }
        if dropped { persist(cacheStore, itemsByAccount) }
        if assistantAccount == nil || dropped, brief != nil || !bundleSummaries.isEmpty || !triageSuggestions.isEmpty {
            if assistantAccount == nil { store(nil) }
            bundleSummaries = [:]
            persist(bundleSummariesStore, bundleSummaries)
            triageSuggestions = []
        }
    }

    /// The only way to build a plugin: it must be allowed, and it can only reach its declared hosts.
    private func sourcePlugin(_ account: Account, secrets: [String: String]) throws -> any SourcePlugin {
        try PluginRegistry.make(account, secrets: secrets, http: guardedClient(for: account))
    }

    /// Built with its model fields kept within the organization's allowed models, if any.
    private func assistantPlugin(_ account: Account, secrets: [String: String]) throws -> any AssistantPlugin {
        var constrained = account
        let defaults = Dictionary(
            (PluginRegistry.manifest(account.pluginID)?.fields ?? []).map { ($0.key, $0.defaultValue) },
            uniquingKeysWith: { first, _ in first }
        )
        constrained.settings = assistantPolicy.constrained(account.settings, defaults: defaults)
        if account.pluginID == ClaudeCodePlugin.manifest.id,
            let path = ManagedPolicy.claudeCodePath(environment.managed)
        {
            constrained.settings["path"] = path
        }
        return try PluginRegistry.makeAssistant(constrained, secrets: secrets, http: guardedClient(for: account))
    }

    private func guardedClient(for account: Account) throws -> HTTPClient {
        guard let manifest = PluginRegistry.manifest(account.pluginID) else {
            throw PluginError.unknownPlugin(account.pluginID)
        }
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
            guard let manifest = PluginRegistry.manifest(account.pluginID), policy.allows(manifest) else {
                return false
            }
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
        let items = assistantPolicy.items(layout.pinned + layout.groups.flatMap(\.items))
        do {
            let assistant = try assistantPlugin(account, secrets: environment.secrets.secrets(for: account.id))
            store(try await assistant.brief(items, now: .now))
        } catch {
            briefError = error.localizedDescription
        }
    }

    /// Summarizes one bundle with the assistant's lighter model, reusing a fresh summary of the same items.
    func summarize(key: String, topic: String, items: [InboxItem]) async {
        let items = assistantPolicy.items(items)
        guard let account = assistantAccount, !items.isEmpty, !summarizing.contains(key) else { return }
        if bundleSummaries[key]?.isFresh(for: items, cacheMinutes: preferences.briefCacheMinutes, now: .now) == true {
            return
        }
        summarizing.insert(key)
        bundleSummaryErrors[key] = nil
        defer { summarizing.remove(key) }
        do {
            let assistant = try assistantPlugin(account, secrets: environment.secrets.secrets(for: account.id))
            let text = try await assistant.digest(items, topic: topic, now: .now)
            bundleSummaries[key] = BundleSummary(text: text, items: items)
            persist(bundleSummariesStore, bundleSummaries)
        } catch {
            bundleSummaryErrors[key] = error.localizedDescription
        }
    }

    var isBriefFresh: Bool {
        brief?.isFresh(cacheMinutes: preferences.briefCacheMinutes, now: .now) ?? false
    }

    private func store(_ brief: Brief?) {
        self.brief = brief
        persist(briefStore, brief)
    }

    func item(_ id: String) -> InboxItem? { itemIndex.byID[id] }

    /// What the rows look up, built once per change of the inbox rather than by every row: items by id, and
    /// by repo or channel (where the waiting assistant finds reviewers). Readers observe `layoutVersion`.
    private struct ItemIndex {
        var byID: [String: InboxItem]
        var byPlace: [String: [InboxItem]]
    }
    @ObservationIgnored private var cachedIndex: (version: Int, index: ItemIndex)?

    private var itemIndex: ItemIndex {
        let version = layoutVersion
        if let cachedIndex, cachedIndex.version == version { return cachedIndex.index }
        let items = allItems
        let index = ItemIndex(
            byID: Dictionary(items.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first }),
            byPlace: Dictionary(grouping: items, by: \.context.contextKey)
        )
        cachedIndex = (version, index)
        return index
    }

    // MARK: Lifecycle

    func start() {
        enforcePolicy()
        refreshInsights()
        recomputeLinks()
        if !isDemo {
            environment.network.observe { [weak self] online in self?.networkChanged(online: online) }
            startPolling()
            scheduleUpdates()
        }
        ticking = Task { [weak self] in
            while !Task.isCancelled {
                self?.tick()
                try? await Task.sleep(for: .seconds(20))
            }
        }
    }

    /// What the footer says about the sources.
    var sourcesHealth: SourcesHealth { SourcesHealth(failures: errors, offline: isOffline) }

    /// The network came and went: offline, nothing is refreshed; back online, everything is, at once. Failures that
    /// were only the missing network go.
    func networkChanged(online: Bool) {
        let wasOffline = isOffline
        isOffline = !online
        if online, wasOffline {
            errors = errors.filter { $0.value.kind != .offline }
            Task { await refresh(manual: true) }
        }
    }

    /// The organization can turn updating off entirely, or on for everyone.
    var updatesAllowed: Bool { ManagedPolicy.updatesAllowed(environment.managed) }

    private func scheduleUpdates() {
        guard !isDemo else { return }
        updater.schedule(enabled: updatesAllowed && effectivePreferences.checkForUpdates)
    }

    /// Check now: the user asked, so it runs even with automatic checks off, unless the organization forbids it.
    func checkForUpdates() async {
        guard updatesAllowed, !isDemo else { return }
        await updater.check()
    }

    /// Accounts that failed lately, and when to ask them again.
    @ObservationIgnored private var backoff: [UUID: Backoff] = [:]

    private func startPolling() {
        polling?.cancel()
        let interval = Double(max(1, effectivePreferences.refreshMinutes) * 60)
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

    /// `manual`: asked for by the user (the button, ⌘R, the menu), which doesn't wait out a slow-down (`Backoff`).
    func refresh(manual: Bool = false) async {
        guard !isRefreshing else { return }
        isRefreshing = true
        defer { isRefreshing = false }

        let classifier = classifier
        let now = Date.now
        let interval = Double(max(1, effectivePreferences.refreshMinutes) * 60)
        // An account that keeps failing, or is rate-limited, sits this one out and keeps its items and its error.
        // Offline, every tool would fail the same way: wait for the network (`networkChanged`).
        guard !isOffline else { return }
        enforcePolicy()
        let policy = policy
        let due = sourceAccounts.filter { account in
            PluginRegistry.manifest(account.pluginID).map(policy.allows) == true
        }.filter { !(backoff[$0.id]?.waits(at: now, manual: manual) ?? false) }
        let jobs = due.map { account in
            (account.id, Result { try sourcePlugin(account, secrets: environment.secrets.secrets(for: account.id)) })
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
                case .success(let snapshot):
                    backoff[id] = nil
                    apply(snapshot, to: id)
                case .failure(let error):
                    let failure = SourceFailure(error)
                    errors[id] = failure
                    // Being offline isn't the tool's fault: no slow-down for it.
                    if failure.kind == .offline { continue }
                    var next = backoff[id] ?? Backoff()
                    if case HTTPError.rateLimited(let until) = error {
                        next.failed(
                            at: .now, interval: interval,
                            rateLimitedUntil: until ?? Date.now.addingTimeInterval(interval))
                    } else {
                        next.failed(at: .now, interval: interval)
                    }
                    backoff[id] = next
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
        remarks[accountID] = snapshot.remarks.isEmpty ? nil : snapshot.remarks
        itemsByAccount[accountID] = snapshot.items

        if let index = accounts.firstIndex(where: { $0.id == accountID }), accounts[index].identity != snapshot.identity
        {
            accounts[index].identity = snapshot.identity
            persist(accountsStore, accounts)
        }

        for notice in ChangeDetector.notices(previous: previous, current: snapshot.items) {
            let wanted = notice.kind == .arrival ? preferences.notifyArrivals : preferences.notifyStatusChanges
            if wanted { environment.notifications.post(respectingPrivacy(notice, from: account(accountID)?.pluginID)) }
        }
        wakeSnoozedItems(touchedIn: snapshot.items)
    }

    private func wakeSnoozedItems(touchedIn items: [InboxItem]) {
        guard preferences.wakeOnActivity else { return }
        for item in items {
            guard let snooze = states[item.id]?.snooze, snooze.mode == .hide, snooze.fingerprint != item.fingerprint
            else { continue }
            update(item) { $0.snooze = nil }
            environment.notifications.cancel(item.id)
        }
    }

    /// Wakes up items whose snooze is over. The notification itself was scheduled with the system. The layout's
    /// time moves only when an item expired or the day changed since.
    func tick(at clock: Date = .now) {
        if Self.layoutTimeChanged(from: now, to: clock, items: allItems) { now = clock }
        var changed = false
        for (id, state) in states {
            guard let snooze = state.snooze, snooze.until <= clock else { continue }
            states[id]?.snooze = nil
            states[id]?.remindedAt = clock
            states[id]?.done = nil
            changed = true
        }
        if changed {
            persist(statesStore, states)
            refreshInsights()
        }
    }

    /// Whether moving the layout's time from `old` to `new` changes the layout: an item expired in between (a Slack
    /// event reminder once the event starts), or a new day began (what's due today).
    static func layoutTimeChanged(from old: Date, to new: Date, items: [InboxItem], calendar: Calendar = .current)
        -> Bool
    {
        if !calendar.isDate(old, inSameDayAs: new) { return true }
        return items.contains { item in item.expires.map { $0 > old && $0 <= new } ?? false }
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
        if !inBrowser, effectivePreferences.openInApps, let app = item.appURL, LinkPolicy.isAppLink(app),
            environment.canOpen(app), environment.open(app)
        {
            // Opened in the app.
        } else if let url = webLink(for: item) {
            _ = environment.open(url)
        }
        update(item) { $0.remindedAt = nil }
        learnHandled(item)
    }

    /// The item's web page, if it's one Remora may open (see `LinkPolicy`).
    func webLink(for item: InboxItem) -> URL? {
        guard let url = item.url else { return nil }
        let account = accounts.first { $0.id == item.accountID }
        return LinkPolicy.isWebLink(url, httpHosts: LinkPolicy.httpHosts(of: account)) ? url : nil
    }

    // MARK: Waiting

    @ObservationIgnored let waitingAssistant = WaitingAssistant()

    /// Reviewer suggestions or a nudge for one of your MRs that's waiting.
    func waitingHelp(_ item: InboxItem) -> WaitingAssistant.Help? {
        // Reviewers come from the same repo: only its items, not the whole inbox per row.
        waitingAssistant.help(
            for: item, among: itemIndex.byPlace[item.context.contextKey] ?? [], me: account(item.accountID)?.identity,
            now: .now)
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

    /// What the last Done, Mark all as done or Clear changed, for its Undo: shown a few seconds, ⌘Z.
    struct UndoPoint: Identifiable {
        let id = UUID()
        /// "Marked as done", "3 marked as done", already translated.
        let message: String
        fileprivate let states: [String: ItemState]
        fileprivate let reminders: [InboxItem]
        fileprivate let history: [SnoozeRecord]
        fileprivate let records: [ActionRecord]
    }

    private(set) var undoPoint: UndoPoint?

    private func remember(_ message: String) {
        undoPoint = UndoPoint(
            message: message, states: states, reminders: reminders, history: snoozeHistory, records: actionRecords)
    }

    /// Puts back what the last undoable action changed, and the snoozes' return notifications it cancelled.
    func undo() {
        guard let point = undoPoint else { return }
        undoPoint = nil
        let cancelled = point.states.filter { id, state in state.snooze != nil && states[id]?.snooze != state.snooze }
        states = point.states
        reminders = point.reminders
        snoozeHistory = point.history
        actionRecords = point.records
        ranker = PersonalRanker(records: actionRecords)
        persist(statesStore, states)
        persist(remindersStore, reminders)
        persist(historyStore, snoozeHistory)
        persist(learningStore, actionRecords)
        for (id, state) in cancelled {
            if let snooze = state.snooze, snooze.until > .now, let item = item(id) {
                scheduleReturn(of: item, at: snooze.until, note: snooze.note)
            }
        }
        refreshInsights()
    }

    /// The Undo offer goes away after a few seconds; a later one replaces it.
    func forgetUndo(_ id: UUID) {
        if undoPoint?.id == id { undoPoint = nil }
    }

    func toggleDone(_ item: InboxItem) {
        if state(of: item).done == nil { remember(L("Marked as done")) }
        markDone(item)
    }

    private func markDone(_ item: InboxItem) {
        if item.bundle == .reminders, state(of: item).done == nil {
            removeReminder(item)
            return
        }
        let markingDone = state(of: item).done == nil
        if markingDone, item.bundle == .reviews, let started = state(of: item).startedAt {
            learnReviewTime(item, startedAt: started)
        }
        update(item) { state in
            state.done = markingDone ? .init(at: .now, fingerprint: item.fingerprint) : nil
            state.remindedAt = nil
            state.snooze = nil
            state.startedAt = nil
        }
        environment.notifications.cancel(item.id)
        if markingDone { learnHandled(item) }
        if markingDone, let index = snoozeHistory.lastIndex(where: { $0.itemID == item.id && $0.doneAt == nil }) {
            snoozeHistory[index].doneAt = .now
            persist(historyStore, snoozeHistory)
        }
        refreshInsights()
    }

    /// A review started then done: its real duration against the plain estimate, the last 50 kept.
    private func learnReviewTime(_ item: InboxItem, startedAt: Date) {
        guard let estimate = ReviewPrep(item)?.estimatedMinutes else { return }
        let actual = Int((Date.now.timeIntervalSince(startedAt) / 60).rounded())
        reviewTimings = Array((reviewTimings + [ReviewTiming(estimated: estimate, actual: actual)]).suffix(50))
        persist(reviewTimesStore, reviewTimings)
    }

    func clear(_ items: [InboxItem]) {
        remember(items.count == 1 ? L("Cleared") : L("Done items cleared"))
        for item in items {
            update(item) { $0.done?.clearedAt = .now }
        }
    }

    /// Mark all as done.
    func sweep(_ items: [InboxItem]) {
        remember(L("%lld item marked as done", plural: "%lld items marked as done", items.count))
        items.forEach(markDone)
    }

    /// Marks the item as the one being worked on: it moves to the In progress view and the menu bar says so.
    func start(_ item: InboxItem) {
        update(item) { state in
            state.startedAt = .now
            state.snooze = nil
            state.remindedAt = nil
            state.done = nil
        }
        environment.notifications.cancel(item.id)
        refreshInsights()
    }

    /// Puts a started item back where it was before.
    func stop(_ item: InboxItem) {
        update(item) { $0.startedAt = nil }
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
            state.startedAt = nil
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
        environment.notifications.cancel(item.id)
        environment.notifications.schedule(
            respectingPrivacy(
                Notice(
                    kind: .reminder,
                    itemID: item.id,
                    title: L(item.bundle == .reminders ? "Reminder" : "Back in your inbox"),
                    subtitle: item.context,
                    body: [item.title, note].compactMap { $0 }.joined(separator: "\n"),
                    url: item.url
                ), from: item.pluginID), at: date)
    }

    /// Hides what was written when the user doesn't want that integration's content on screen.
    private func respectingPrivacy(_ notice: Notice, from pluginID: String?) -> Notice {
        guard let pluginID, effectivePreferences.hiddenContentPlugins.contains(pluginID) else { return notice }
        return notice.redacted()
    }

    func unsnooze(_ item: InboxItem) {
        update(item) { $0.snooze = nil }
        environment.notifications.cancel(item.id)
        refreshInsights()
    }

    // MARK: Smart snooze

    /// Times this item was snoozed in the last 30 days.
    /// Snoozes of each item in the last 30 days, counted once per change of the history and once a day.
    func snoozeCount(_ item: InboxItem) -> Int {
        let history = snoozeHistory
        let today = Calendar.current.startOfDay(for: .now)
        if let cachedSnoozeCounts, cachedSnoozeCounts.day == today { return cachedSnoozeCounts.counts[item.id] ?? 0 }
        let since = Date.now.addingTimeInterval(-30 * 86_400)
        let counts = Dictionary(history.filter { $0.at > since }.map { ($0.itemID, 1) }, uniquingKeysWith: +)
        cachedSnoozeCounts = (today, counts)
        return counts[item.id] ?? 0
    }

    @ObservationIgnored private var cachedSnoozeCounts: (day: Date, counts: [String: Int])?

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

    /// Patterns in the snoozed pile, worked out off the main thread: clustering compares every pair of titles.
    /// A newer request wins over a slower, older one.
    func refreshInsights() {
        let (advisor, snoozed, states, history) = (advisor, layout.snoozed, states, snoozeHistory)
        insightsGeneration &+= 1
        let generation = insightsGeneration
        insightsTask = Task {
            let insights = await Task.detached {
                advisor.insights(snoozed: snoozed, states: states, history: history, now: .now)
            }.value
            guard generation == self.insightsGeneration else { return }
            self.insights = insights
        }
    }

    @ObservationIgnored private var insightsGeneration = 0
    /// The latest computation, for tests to wait on.
    @ObservationIgnored private(set) var insightsTask: Task<Void, Never>?

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
        for item in items { reschedule(item, to: earliest) }
    }

    /// Asks the assistant what to do with each snoozed item, reusing a fresh answer for the same pile.
    func triage() async {
        guard let account = assistantAccount, !isTriaging else { return }
        let snoozed = layout.snoozed
        let ids = snoozed.map(\.id).sorted()
        if let key = triageKey, key.ids == ids,
            key.at.addingTimeInterval(Double(preferences.briefCacheMinutes) * 60) > .now, !triageSuggestions.isEmpty
        {
            return
        }
        let items = assistantPolicy.items(snoozed).compactMap { item in
            state(of: item).snooze.map { SnoozedItem(item: item, snooze: $0, times: snoozeCount(item)) }
        }
        isTriaging = true
        triageError = nil
        defer { isTriaging = false }
        do {
            let assistant = try assistantPlugin(account, secrets: environment.secrets.secrets(for: account.id))
            triageSuggestions = try await assistant.triage(items, now: .now)
            triageKey = (ids, .now)
        } catch {
            triageError = error.localizedDescription
        }
    }

    func apply(_ suggestions: [TriageSuggestion]) {
        var opened = false
        for suggestion in suggestions {
            guard let item = item(suggestion.id) else { continue }
            switch suggestion.action {
            case .keep: break
            case .reschedule: if let until = suggestion.until { reschedule(item, to: until) }
            case .done: toggleDone(item)
            case .now:
                // Every "now" item comes back to the inbox; only the first opens, never a burst of links.
                unsnooze(item)
                if !opened { open(item) }
                opened = true
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
        case .start:
            start(item)
        }
    }

    func addReminder(_ title: String, at date: Date) {
        let item = InboxItem(
            id: "reminder/\(UUID().uuidString)",
            accountID: UUID(),
            pluginID: "reminders",
            bundle: .reminders,
            title: title,
            // Stored in English and translated when shown (`shownContext`).
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
        environment.notifications.cancel(item.id)
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
            if preferences.wholeInboxBrief {
                // The first brief doubles as the connection test.
                store(try await assistant.brief(assistantPolicy.items(layout.groups.flatMap(\.items)), now: .now))
            } else {
                // The brief is off: test with one made-up item, so no inbox content leaves the Mac yet.
                _ = try await assistant.digest([Self.connectionTest], topic: "Test", now: .now)
            }
            try environment.secrets.save(secrets, for: account.id)
            accounts.removeAll { PluginRegistry.isAssistant($0.pluginID) }
            accounts.append(account)
            persist(accountsStore, accounts)
            return
        }
        let plugin = try sourcePlugin(account, secrets: secrets)
        var snapshot = try await plugin.fetch()
        let (classifier, fetched) = (classifier, snapshot.items)
        snapshot.items = await Task.detached { fetched.map(classifier.classify) }.value
        // The same person on the same tool and host, connected again: that account takes the new token and
        // keeps its id, so Done, snoozes and pins stay. A new name replaces the old one.
        if let existing = accounts.first(where: { $0.isSame(as: account, identity: snapshot.identity) }) {
            snapshot.items = snapshot.items.map { $0.moved(from: account.id, to: existing.id) }
            try environment.secrets.save(secrets, for: existing.id)
            if let name, let index = accounts.firstIndex(where: { $0.id == existing.id }) {
                accounts[index].name = name
                persist(accountsStore, accounts)
            }
            backoff[existing.id] = nil
            apply(snapshot, to: existing.id)
            persist(cacheStore, itemsByAccount)
            return
        }
        try environment.secrets.save(secrets, for: account.id)
        accounts.append(account)
        persist(accountsStore, accounts)
        apply(snapshot, to: account.id)
        persist(cacheStore, itemsByAccount)
    }

    /// What connecting an assistant sends when the whole-inbox brief is off: nothing from the inbox.
    static let connectionTest = InboxItem(
        id: "remora/connection-test", accountID: UUID(), pluginID: "remora", bundle: .reminders,
        title: "Connection test", context: "Remora", date: .now
    )

    /// Removes everything Remora stored on this Mac: accounts, tokens, cache, history, settings.
    /// Everything else is erased even when the Keychain refuses; then that refusal is thrown, so it is said.
    func eraseLocalData() throws {
        undoPoint = nil
        storageIssues = []
        updater.forget()
        let keychain = Result { try environment.secrets.deleteAll() }
        environment.notifications.removeAll()
        accounts = []
        itemsByAccount = [:]
        reminders = []
        states = [:]
        errors = [:]
        remarks = [:]
        brief = nil
        bundleSummaries = [:]
        snoozeHistory = []
        actionRecords = []
        reviewTimings = []
        ranker = PersonalRanker(records: [])
        links = [:]
        triageSuggestions = []
        insights = []
        syncedAccounts = []
        preferences = Preferences()
        if let files = try? FileManager.default.contentsOfDirectory(
            at: environment.folder, includingPropertiesForKeys: nil)
        {
            for file in files { try? FileManager.default.removeItem(at: file) }
        }
        environment.removeURLCaches()
        try keychain.get()
    }

    /// The token goes first: when the Keychain refuses, the account stays, with the reason under it.
    func disconnect(_ account: Account) {
        do {
            try environment.secrets.delete(account.id)
        } catch {
            errors[account.id] = SourceFailure(kind: .other, message: error.localizedDescription)
            return
        }
        undoPoint = nil
        accounts.removeAll { $0.id == account.id }
        if PluginRegistry.isAssistant(account.pluginID) {
            store(nil)
            bundleSummaries = [:]
            persist(bundleSummariesStore, bundleSummaries)
        }
        itemsByAccount[account.id] = nil
        errors[account.id] = nil
        remarks[account.id] = nil
        syncedAccounts.remove(account.id)
        environment.notifications.remove(itemsWithPrefix: "\(account.id.uuidString)/")
        persist(accountsStore, accounts)
        persist(cacheStore, itemsByAccount)
        pruneStates()
    }
}

extension InboxModel {
    fileprivate func persist<Value>(_ store: JSONStore<Value>, _ value: Value) {
        guard !isDemo else { return }
        // A failed write is said in Settings → Privacy, once per file, and cleared when it works again.
        let name = store.url.lastPathComponent
        do {
            try store.save(value)
            storageIssues.removeAll { $0.file == name && !$0.corrupt }
        } catch {
            storageIssues.removeAll { $0.file == name && !$0.corrupt }
            storageIssues.append(StorageIssue(file: name, corrupt: false, message: error.localizedDescription))
        }
    }
}

extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
}

extension Account {
    /// The same person on the same tool and server: what connecting twice would duplicate.
    func isSame(as other: Account, identity: String) -> Bool {
        guard pluginID == other.pluginID, !identity.isEmpty, self.identity?.lowercased() == identity.lowercased() else {
            return false
        }
        // "gitlab.acme.io", "https://gitlab.acme.io/" and an empty field (the default host) compare by host.
        let host = { (account: Account) -> String? in
            guard let value = account.settings["host"]?.trimmingCharacters(in: .whitespaces), !value.isEmpty else {
                return nil
            }
            return URL(string: value.contains("://") ? value : "https://" + value)?.host?.lowercased()
        }
        return host(self) == host(other)
    }
}

extension InboxItem {
    /// The item as the account `to` fetched it: plugins put the account id at the start of item ids.
    func moved(from old: UUID, to new: UUID) -> InboxItem {
        var item = self
        item.accountID = new
        if item.id.hasPrefix(old.uuidString) { item.id = new.uuidString + item.id.dropFirst(old.uuidString.count) }
        return item
    }
}

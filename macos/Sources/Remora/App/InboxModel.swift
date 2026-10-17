import AppKit
import Observation
import RemoraCore
import RemoraPlugins

/// The inbox, as the views see it: a facade over the services it is made of, with the same names on Windows
/// (`inbox/`).
/// - `AccountsService`: the accounts, their tokens and sessions.
/// - `SyncEngine`: failures, slow-downs, the network, notifications held for later.
/// - `SnoozeService`: the history, the advisor, insights.
/// - `AssistantCoordinator`: the brief, summaries and triage.
/// - `LearningService`: the ranker and review times, learned from your actions.
/// - `SpotlightService`: what Spotlight holds.
/// - `Persistence`: the files each of them saves.
/// - `InboxPolicy`: the privacy policy, from the preferences and the organization's settings.
/// This class holds what they share (the items, their states, reminders and preferences), what to fetch and how to
/// store it, the actions on items, and Undo.
@MainActor @Observable
final class InboxModel {
    @ObservationIgnored let accountsService: AccountsService
    @ObservationIgnored let sync: SyncEngine
    @ObservationIgnored let snoozes: SnoozeService
    @ObservationIgnored let assistant: AssistantCoordinator
    @ObservationIgnored let learning: LearningService
    @ObservationIgnored let spotlight: SpotlightService
    @ObservationIgnored let persistence: Persistence

    var accounts: [Account] { accountsService.accounts }
    private(set) var itemsByAccount: [UUID: [InboxItem]] {
        didSet {
            layoutChanged()
            updateSpotlight()
        }
    }
    /// What agents asked through `remora mcp`, oldest first (V15-01), for Settings.
    private(set) var agentReads: [AgentRead] = []
    private(set) var reminders: [InboxItem] {
        didSet {
            layoutChanged()
            updateSpotlight()
        }
    }
    private(set) var states: [String: ItemState] { didSet { layoutChanged() } }
    struct StorageIssue: Equatable, Identifiable {
        var file: String
        /// Read and kept aside at launch, rather than failing to save.
        var corrupt: Bool
        var message: String
        var id: String { file + (corrupt ? "/read" : "/write") }
    }

    /// Files that couldn't be read (kept aside) or saved, for Settings → Privacy.
    private(set) var storageIssues: [StorageIssue] = []
    /// When the passing of time changed the layout: it moves only when an item expires or a new day starts
    /// (what's due today). Snoozes ending change `states` instead. The layout itself is computed at the real time.
    private(set) var now = Date.now { didSet { layoutChanged() } }
    /// Item ID → items from other tools about the same thing. Computed in the background after each refresh.
    private(set) var links: [String: [String]] = [:] { didSet { layoutChanged() } }
    var query = ""
    /// Bumped each time the menu bar popover opens, so the inbox can pick its starting tab.
    var popoverOpenings = 0
    /// The open-the-inbox shortcut is held by another app (MISS-04): Settings says so.
    var openShortcutTaken = false
    /// The tab a `remora://open?tab=…` link asked for (`myTurn`, `waiting`…), until the inbox shows it (IDEA-06).
    var requestedTab: String?
    var preferences: Preferences {
        didSet {
            persistence.save(preferences, to: persistence.preferences)
            if oldValue.wakeOnActivity != preferences.wakeOnActivity { layoutChanged() }
            Myna.textScale = preferences.textSize.scale
            if oldValue.allowedPlugins != preferences.allowedPlugins
                || oldValue.allowExternalAI != preferences.allowExternalAI
            {
                enforcePolicy()
            }
            if InboxPolicy(preferences: oldValue, managed: environment.managed).effectivePreferences.refreshMinutes
                != effectivePreferences.refreshMinutes
            {
                startPolling()
            }
            if oldValue.checkForUpdates != preferences.checkForUpdates { scheduleUpdates() }
            if oldValue.spotlight != preferences.spotlight
                || oldValue.hiddenContentPlugins != preferences.hiddenContentPlugins
            {
                updateSpotlight()
            }
        }
    }

    @ObservationIgnored private let environment: AppEnvironment
    let updater: Updater
    @ObservationIgnored private let http: HTTPClient
    /// Keywords, then on-device sentence embeddings. Slow on first sight of a message, so it runs off the main thread.
    @ObservationIgnored private let classifier: VerbClassifier
    @ObservationIgnored private let linkFinder = LinkFinder(
        similarity: [
            KeywordSimilarity(), RareTokenSimilarity(),
            EmbeddingSimilarity(threshold: EmbeddingSimilarity.linkThreshold),
        ]
    )
    @ObservationIgnored private var ticking: Task<Void, Never>?
    @ObservationIgnored private let isDemo: Bool
    /// Saves the files off the main thread, each once per burst of changes.
    var writer: DiskWriter { persistence.writer }
    /// Inside `batch`: insights are worked out once, at the end.
    @ObservationIgnored private var batchDepth = 0
    @ObservationIgnored private var batchNeedsInsights = false

    init(environment: AppEnvironment) {
        let embeddings = EmbeddingIntentClassifier()
        classifier = VerbClassifier([KeywordIntentClassifier(), embeddings])
        // The sentence models load in the background at launch, so the first refresh doesn't wait for them (ML-09).
        Task.detached(priority: .utility) { embeddings.warmUp() }
        self.environment = environment
        http = environment.http
        updater = Updater(system: environment.updates)
        isDemo = environment.demo
        let files = Persistence(folder: environment.folder, demo: environment.demo)
        persistence = files
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
        let preferences = read(files.preferences) ?? Preferences()
        self.preferences = preferences
        Myna.textScale = preferences.textSize.scale
        sync = SyncEngine()
        spotlight = SpotlightService(index: environment.spotlight, demo: environment.demo)
        if isDemo {
            accountsService = AccountsService(accounts: [], secrets: environment.secrets, persistence: files)
            let demoItems = DemoData.items().mapValues { $0.map(VerbClassifier().classify) }
            itemsByAccount = demoItems
            assistant = AssistantCoordinator(brief: DemoData.brief, bundleSummaries: [:], persistence: files)
            reminders = []
            let demo = DemoData.snoozes(items: demoItems.values.flatMap { $0 })
            states = demo.states
            snoozes = SnoozeService(history: demo.history, persistence: files)
            learning = LearningService(records: [], reviewTimings: [], persistence: files)
        } else {
            accountsService = AccountsService(
                accounts: read(files.accounts) ?? [], secrets: environment.secrets, persistence: files)
            // Fetched again at the next refresh: a broken cache or brief is just started again. Cached items keep
            // the verbs the refresh gave them, with the full classifier (ML-16).
            itemsByAccount = files.cache.load() ?? [:]
            reminders = read(files.reminders) ?? []
            states = read(files.states) ?? [:]
            assistant = AssistantCoordinator(
                brief: files.brief.load() ?? nil, bundleSummaries: files.bundleSummaries.load() ?? [:],
                usage: files.assistantUsage.load() ?? AssistantUsage(), persistence: files)
            snoozes = SnoozeService(history: read(files.history) ?? [], persistence: files)
            learning = LearningService(
                records: read(files.learning) ?? [], reviewTimings: read(files.reviewTimes) ?? [], persistence: files)
            agentReads = files.agentReads.load() ?? []
            environment.removeURLCaches()
        }
        storageIssues = issues
        writer.onResult = { [weak self] file, failure in self?.recordWrite(file, failure: failure) }
        learning.rankerChanged = { [weak self] in self?.layoutChanged() }
        assistant.dailyLimit = { [weak self] in self?.effectivePreferences.assistantDailyLimit ?? 0 }
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
        let ranker = learning.ranker
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

    /// One link search at a time: a new one cancels the last, and only the newest result is kept, so an older
    /// search finishing late can't bring back links to items already gone (PERF-18).
    private func recomputeLinks() {
        let items = allItems
        let finder = linkFinder
        linksGeneration &+= 1
        let generation = linksGeneration
        linksTask?.cancel()
        linksTask = Task {
            let links = await Task.detached { finder.links(items) }.value
            guard !Task.isCancelled, generation == self.linksGeneration else { return }
            self.links = links
        }
    }

    @ObservationIgnored private var linksGeneration = 0
    /// The latest link search, for tests to wait on.
    @ObservationIgnored private(set) var linksTask: Task<Void, Never>?

    func state(of item: InboxItem) -> ItemState { states[item.id] ?? ItemState() }

    func account(_ id: UUID) -> Account? { accountsService.account(id) }

    func rename(_ account: Account, to name: String) {
        accountsService.rename(account.id, to: name.trimmingCharacters(in: .whitespaces).nilIfEmpty)
    }

    /// The account's name, shown on items only when several accounts of the same plugin are connected.
    func accountLabel(for item: InboxItem) -> String? {
        guard accounts.filter({ $0.pluginID == item.pluginID }).count > 1, let account = account(item.accountID) else {
            return nil
        }
        return account.name ?? account.identity
    }

    var sourceAccounts: [Account] { accountsService.sourceAccounts }

    /// The connected assistant, only when the privacy policy allows it, its AI server included.
    var assistantAccount: Account? {
        accounts.first { PluginRegistry.isAssistant($0.pluginID) && privacy.refusal(for: $0) == nil }
    }

    // MARK: Privacy

    /// The privacy policy, from the preferences as they are now (`InboxPolicy`).
    var privacy: InboxPolicy { InboxPolicy(preferences: preferences, managed: environment.managed) }

    var policy: CompliancePolicy { privacy.compliance }

    /// What Remora acts on: your preferences with the organization's settings in place.
    var effectivePreferences: Preferences { privacy.effectivePreferences }

    /// What the assistant may read and which models it may use.
    var assistantPolicy: AssistantPolicy { privacy.assistantPolicy }

    /// What the privacy policy refuses leaves the screen and the disk at once, not at the next successful
    /// refresh: a refused tool's cached items, its notifications and the AI summaries that may quote them. Its account
    /// stays, saying why and how to delete its token. Without an allowed assistant, the brief, summaries and triage
    /// suggestions go. Runs at launch, at each refresh (a new MDM profile) and when Privacy settings change.
    func enforcePolicy() {
        guard !isDemo else { return }
        var dropped = false
        for account in accounts {
            guard let refusal = privacy.refusal(for: account) else { continue }
            sync.setFailure(
                SourceFailure(kind: .other, message: L("%@ Remove the account to delete its token.", refusal)),
                for: account.id)
            if itemsByAccount[account.id] != nil {
                itemsByAccount[account.id] = nil
                sync.unsynced(account.id)
                environment.notifications.remove(itemsWithPrefix: InboxItem.accountPrefix(account.id))
                dropped = true
            }
        }
        if dropped { persistence.save(itemsByAccount, to: persistence.cache) }
        let noAssistant = assistantAccount == nil
        if noAssistant || dropped { assistant.forget(brief: noAssistant) }
        // A tool the organization now hides or blocks leaves Spotlight too.
        updateSpotlight()
    }

    /// What Spotlight may hold: reminders, and the items of tools the policy allows (`SpotlightService`).
    private func updateSpotlight() {
        guard !isDemo else { return }
        var names = Dictionary(
            PluginRegistry.manifests.map { ($0.id, $0.name) }, uniquingKeysWith: { first, _ in first })
        names[SpotlightIndex.remindersDomain] = L("Reminder")
        let privacy = privacy
        let items = allItems.filter { item in
            item.pluginID == SpotlightIndex.remindersDomain
                || account(item.accountID).map { privacy.refusal(for: $0) == nil } == true
        }
        spotlight.update(items: items, preferences: privacy.effectivePreferences, sourceNames: names)
    }

    /// The only way to build a plugin: it must be allowed, and it can only reach its declared hosts. A refresh
    /// reuses what the account kept (PERF-09); connecting starts from nothing, so the token is really tested.
    private func sourcePlugin(_ account: Account, secrets: [String: String], remembering: Bool = true) throws
        -> any SourcePlugin
    {
        let account = privacy.effective(account)
        guard remembering else {
            return try PluginRegistry.make(account, secrets: secrets, http: guardedClient(for: account))
        }
        let session = accountsService.session(for: account, secrets: secrets)
        return try PluginRegistry.make(
            account, secrets: secrets,
            http: guardedClient(for: account, through: ConditionalHTTPClient(http, store: session.etags)),
            cache: session.cache)
    }

    /// Built with its model fields kept within the organization's allowed models, if any.
    private func assistantPlugin(_ account: Account, secrets: [String: String]) throws -> any AssistantPlugin {
        let privacy = privacy
        return try PluginRegistry.makeAssistant(
            privacy.constrained(assistant: account), secrets: secrets,
            http: guardedClient(for: privacy.effective(account)))
    }

    /// `base` is what the guard lets requests through to: the network, or a layer in front of it.
    private func guardedClient(for account: Account, through base: HTTPClient? = nil) throws -> HTTPClient {
        try privacy.guardedClient(for: account, through: base ?? http)
    }

    /// Avatars load only from the hosts of allowed, connected tools.
    func allowsImage(_ url: URL?) -> Bool { privacy.allowsImage(url, from: sourceAccounts) }

    /// The picture at `url` when the allowlist lets it load (`cachedImage` when it already has).
    func image(at url: URL?) async -> NSImage? {
        guard let url, allowsImage(url) else { return nil }
        return await environment.images.image(at: url)
    }

    func cachedImage(_ url: URL?) -> NSImage? {
        guard let url, allowsImage(url) else { return nil }
        return environment.images.cached(url)
    }

    // MARK: Assistant

    /// Writes a new brief, unless the current one is still fresh (Settings → General → Assistant).
    func makeBrief() async {
        guard preferences.wholeInboxBrief, let account = assistantAccount, !assistant.isBriefing, !isBriefFresh else {
            return
        }
        await assistant.makeBrief(
            from: briefItems, assistant: { try assistantPlugin(for: account) }, stillAnswers: { stillAnswers(account) })
    }

    /// Summarizes one bundle with the assistant's lighter model, reusing a fresh summary of the same items.
    func summarize(key: String, topic: String, items: [InboxItem]) async {
        let items = assistantPolicy.items(items)
        guard let account = assistantAccount, !items.isEmpty else { return }
        await assistant.summarize(
            key: key, topic: topic, items: items, cacheMinutes: preferences.briefCacheMinutes,
            assistant: { try assistantPlugin(for: account) }, stillAnswers: { stillAnswers(account) })
    }

    /// The assistant for its account, with its token.
    private func assistantPlugin(for account: Account) throws -> any AssistantPlugin {
        try assistantPlugin(account, secrets: accountsService.secrets(for: account.id))
    }

    /// Whether an answer from this account may still be kept. The assistant can take a minute or more: an answer that
    /// comes back after Erase, a disconnect or a policy that turns AI off is dropped (SEC-27).
    private func stillAnswers(_ account: Account) -> Bool {
        assistantAccount?.id == account.id
    }

    /// What a brief is written from: the task in progress (marked so), the pinned items and My turn, as the
    /// assistant may see them (AI-20).
    var briefItems: [InboxItem] {
        let layout = layout
        let started = layout.inProgress.map { item in
            var item = item
            item.badges.append(Badge(id: .inProgress, label: L("In progress"), symbol: "play.fill", tone: .accent))
            return item
        }
        return assistantPolicy.items(started + layout.pinned + layout.groups.flatMap(\.items))
    }

    var isBriefFresh: Bool {
        assistant.isBriefFresh(for: briefItems, cacheMinutes: preferences.briefCacheMinutes)
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
        updateSpotlight()
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

    /// The network came and went: offline, nothing is refreshed; back online, everything is, at once. Failures that
    /// were only the missing network go.
    func networkChanged(online: Bool) {
        if sync.networkChanged(online: online) {
            Task { await refresh(manual: true) }
        }
    }

    private func scheduleUpdates() {
        guard !isDemo else { return }
        updater.schedule(enabled: privacy.updatesAllowed && effectivePreferences.checkForUpdates)
    }

    /// Check now: the user asked, so it runs even with automatic checks off, unless the organization forbids it.
    func checkForUpdates() async {
        guard privacy.updatesAllowed, !isDemo else { return }
        await updater.check()
    }

    private func startPolling() {
        let interval = Double(max(1, effectivePreferences.refreshMinutes) * 60)
        sync.poll(every: interval) { [weak self] in await self?.refresh() }
    }

    func refreshIfStale() {
        guard (sync.lastRefresh ?? .distantPast).timeIntervalSinceNow < -60 else { return }
        Task { await refresh() }
    }

    /// `manual`: asked for by the user (the button, ⌘R, the menu), which doesn't wait out a slow-down (`Backoff`).
    /// The sync engine runs the fetches; this says which, and stores what comes back.
    func refresh(manual: Bool = false) async {
        let kept = await sync.refresh(
            manual: manual, interval: Double(max(1, effectivePreferences.refreshMinutes) * 60), classifier: classifier,
            jobs: { waits in
                enforcePolicy()
                let policy = policy
                return sourceAccounts.filter { account in
                    PluginRegistry.manifest(account.pluginID).map(policy.allows) == true && !waits(account.id)
                }.map { account in
                    SyncEngine.FetchJob(
                        id: account.id,
                        plugin: Result { try sourcePlugin(account, secrets: accountsService.secrets(for: account.id)) })
                }
            },
            keeps: { id in account(id).flatMap { privacy.refusal(for: $0) == nil ? $0 : nil } },
            apply: { account, snapshot in
                apply(snapshot, to: account.id)
                await accountsService.saveRenewed(for: account)
            })
        // Nothing to store: in particular, nothing written back after Erase.
        guard kept else { return }
        persistence.save(itemsByAccount, to: persistence.cache)
        pruneStates()
        refreshInsights()
        recomputeLinks()
    }

    private func apply(_ snapshot: SourceSnapshot, to accountID: UUID) {
        // The first sync after launch compares with the cache, if there is one: what arrived while Remora was closed.
        let previous = itemsByAccount[accountID]
        let firstSync = sync.succeeded(accountID, remarks: snapshot.remarks)
        itemsByAccount[accountID] = snapshot.items
        accountsService.setIdentity(snapshot.identity, for: accountID)

        let pluginID = account(accountID)?.pluginID
        var notices = ChangeDetector.notices(previous: previous, current: snapshot.items).filter { notice in
            notice.kind == .arrival ? preferences.notifyArrivals : preferences.notifyStatusChanges
        }
        if firstSync, let account = account(accountID) {
            notices = ChangeDetector.whileClosed(
                notices, accountID: accountID.uuidString,
                accountName: account.name ?? PluginRegistry.manifest(account.pluginID)?.name ?? account.pluginID)
        }
        let shown = notices.map { notice in notice.isSummary ? notice : respectingPrivacy(notice, from: pluginID) }
        // Grouped at set times: what arrived or changed waits for `tick` (IDEA-26).
        for notice in sync.hold(shown, batching: effectivePreferences.batchNotifications) {
            environment.notifications.post(notice)
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
            persistence.save(states, to: persistence.states)
            refreshInsights()
        }
        // What waited, at the first set time since the last batch, or at once when grouping was turned off.
        for notice in sync.release(at: clock, preferences: effectivePreferences) {
            environment.notifications.post(notice)
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
        if states.count != before { persistence.save(states, to: persistence.states) }
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
        learning.handled(item)
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
            message: message, states: states, reminders: reminders, history: snoozes.history, records: learning.records)
    }

    /// Puts back what the last undoable action changed, and the snoozes' return notifications it cancelled.
    func undo() {
        guard let point = undoPoint else { return }
        undoPoint = nil
        let cancelled = point.states.filter { id, state in state.snooze != nil && states[id]?.snooze != state.snooze }
        // Snoozes the action set (a repeating reminder's next time) lose their notification.
        for (id, state) in states where state.snooze != nil && point.states[id]?.snooze == nil {
            environment.notifications.cancel(id)
        }
        states = point.states
        reminders = point.reminders
        snoozes.restore(point.history)
        learning.restore(point.records)
        persistence.save(states, to: persistence.states)
        persistence.save(reminders, to: persistence.reminders)
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
        if state(of: item).done == nil {
            if let next = nextOccurrence(of: item) {
                remember(L("Next reminder %@", next.formatted(.dateTime.weekday(.wide).hour().minute())))
            } else {
                remember(L("Marked as done"))
            }
        }
        markDone(item)
    }

    /// When a repeating reminder comes back after this one is done, if it does.
    private func nextOccurrence(of item: InboxItem) -> Date? {
        guard item.bundle == .reminders else { return nil }
        return item.recurrence?.next(after: .now)
    }

    private func markDone(_ item: InboxItem) {
        if item.bundle == .reminders, state(of: item).done == nil {
            if let next = nextOccurrence(of: item) {
                // Hidden until its next time, like a new reminder; never counted as a snooze.
                update(item) { state in
                    state.done = nil
                    state.remindedAt = nil
                    state.startedAt = nil
                    state.snooze = Snooze(until: next, mode: .hide, fingerprint: item.fingerprint)
                }
                scheduleReturn(of: item, at: next, note: nil)
                refreshInsights()
            } else {
                removeReminder(item)
            }
            return
        }
        let markingDone = state(of: item).done == nil
        if markingDone, item.bundle == .reviews, let started = state(of: item).startedAt {
            learning.reviewed(item, startedAt: started)
        }
        update(item) { state in
            state.done = markingDone ? .init(at: .now, fingerprint: item.fingerprint) : nil
            state.remindedAt = nil
            state.snooze = nil
            state.startedAt = nil
        }
        environment.notifications.cancel(item.id)
        if markingDone { learning.handled(item) }
        if markingDone { snoozes.done(item) }
        refreshInsights()
    }

    func clear(_ items: [InboxItem]) {
        remember(items.count == 1 ? L("Cleared") : L("Done items cleared"))
        batch {
            for item in items {
                // The mark is for the item as it is now: one the tool put in Done (your ✅, a finished conversation)
                // may have no mark, or one from an older message, and must clear too. New activity brings it back.
                update(item) { state in
                    state.done = ItemState.Mark(
                        at: state.done?.at ?? .now, fingerprint: item.fingerprint, clearedAt: .now)
                }
            }
        }
    }

    /// Mark all as done.
    func sweep(_ items: [InboxItem]) {
        remember(L("%lld item marked as done", plural: "%lld items marked as done", items.count))
        batch {
            for item in items { markDone(item) }
        }
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
            learning.deferred(item)
            snoozes.record(item, reason: reason, until: date)
        }
        scheduleReturn(of: item, at: date, note: note)
        refreshInsights()
    }

    func snoozeMany(_ items: [InboxItem], until date: Date, reason: SnoozeReason?, untilNews: Bool = false) {
        remember(L("%lld item snoozed", plural: "%lld items snoozed", items.count))
        batch {
            for item in items {
                snooze(item, until: date, mode: .hide, note: nil, reason: reason, untilNews: untilNews)
            }
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
                    // A reminder's context is "Reminder" again, stored in English: the title says it already.
                    subtitle: item.bundle == .reminders ? "" : item.context,
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
    func snoozeCount(_ item: InboxItem) -> Int { snoozes.count(item) }

    func suggestedReturn(for reason: SnoozeReason) -> Date { snoozes.suggestedReturn(for: reason) }

    func usualReason(for item: InboxItem) -> SnoozeReason? { snoozes.usualReason(for: item) }

    func usualReturn(for item: InboxItem) -> Date? { snoozes.usualReturn(for: item) }

    var visibleInsights: [SnoozeInsight] { snoozes.visibleInsights }

    /// Patterns in the snoozed pile, worked out off the main thread: clustering compares every pair of titles.
    /// A newer request wins over a slower, older one.
    func refreshInsights() {
        guard batchDepth == 0 else {
            batchNeedsInsights = true
            return
        }
        snoozes.refreshInsights(snoozed: layout.snoozed, states: states)
    }

    /// Several actions as one (Mark all as done, Snooze all): insights and the ranker are worked out once, after.
    private func batch(_ actions: () -> Void) {
        batchDepth += 1
        learning.batch(actions)
        batchDepth -= 1
        guard batchDepth == 0, batchNeedsInsights else { return }
        batchNeedsInsights = false
        refreshInsights()
    }
    /// The latest computation, for tests to wait on.
    var insightsTask: Task<Void, Never>? { snoozes.insightsTask }

    func dismiss(_ insight: SnoozeInsight) {
        snoozes.dismiss(insight)
    }

    /// Spreads a pile-up 30 minutes apart from its slot.
    func spread(_ items: [InboxItem], from start: Date) {
        remember(L("%lld return moved", plural: "%lld returns moved", items.count))
        batch {
            for (id, date) in snoozes.advisor.spread(items, from: start) {
                if let item = item(id) { reschedule(item, to: date) }
            }
        }
    }

    /// Brings every item back at the earliest of their return times.
    func alignReturns(_ items: [InboxItem]) {
        guard let earliest = items.compactMap({ state(of: $0).snooze?.until }).min() else { return }
        remember(L("%lld return moved", plural: "%lld returns moved", items.count))
        batch {
            for item in items { reschedule(item, to: earliest) }
        }
    }

    /// Asks the assistant what to do with each snoozed item, reusing a fresh answer for the same pile.
    func triage() async {
        guard let account = assistantAccount, !assistant.isTriaging else { return }
        let snoozed = layout.snoozed
        await assistant.triage(
            ids: snoozed.map(\.id).sorted(),
            items: {
                assistantPolicy.items(snoozed).compactMap { item in
                    state(of: item).snooze.map { SnoozedItem(item: item, snooze: $0, times: snoozeCount(item)) }
                }
            },
            cacheMinutes: preferences.briefCacheMinutes,
            assistant: { try assistantPlugin(for: account) }, stillAnswers: { stillAnswers(account) })
    }

    /// The ticked suggestions, undone together by one Undo.
    func apply(_ suggestions: [TriageSuggestion]) {
        let changes = suggestions.filter { $0.action != .keep }.count
        if changes > 0 { remember(L("%lld change applied", plural: "%lld changes applied", changes)) }
        var opened = false
        batch {
            for suggestion in suggestions {
                guard let item = item(suggestion.id) else { continue }
                switch suggestion.action {
                case .keep: break
                case .reschedule: if let until = suggestion.until { reschedule(item, to: until) }
                case .done: if state(of: item).done == nil { markDone(item) }
                case .now:
                    // Every "now" item comes back to the inbox; only the first opens, never a burst of links.
                    unsnooze(item)
                    if !opened { open(item) }
                    opened = true
                }
            }
        }
        assistant.applied(Set(suggestions.map(\.id)))
    }

    func discardTriage() {
        assistant.discardTriage()
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

    /// A reminder hidden until `date`; with a rule, it comes back after each Done until `until`'s day. Its id, or
    /// nil when the rule ends before it starts.
    @discardableResult
    func addReminder(
        _ title: String, at date: Date, repeating rule: Recurrence.Rule? = nil, until: Date? = nil
    ) -> String? {
        var item = InboxItem(
            id: "reminder/\(UUID().uuidString)",
            accountID: UUID(),
            pluginID: "reminders",
            bundle: .reminders,
            title: title,
            // Stored in English and translated when shown (`shownContext`).
            context: "Reminder",
            date: .now
        )
        if let rule {
            let recurrence = Recurrence(rule: rule, anchor: date, until: until)
            guard !recurrence.endsBeforeStart() else { return nil }
            item.recurrence = recurrence
        }
        reminders.append(item)
        persistence.save(reminders, to: persistence.reminders)
        snooze(item, until: date, mode: .hide, note: nil)
        return item.id
    }

    /// This reminder stays as it is, but won't come back after Done.
    func stopRepeating(_ item: InboxItem) {
        guard let index = reminders.firstIndex(where: { $0.id == item.id }) else { return }
        reminders[index].recurrence = nil
        persistence.save(reminders, to: persistence.reminders)
    }

    private func removeReminder(_ item: InboxItem) {
        reminders.removeAll { $0.id == item.id }
        states[item.id] = nil
        persistence.save(reminders, to: persistence.reminders)
        persistence.save(states, to: persistence.states)
        environment.notifications.cancel(item.id)
    }

    private func update(_ item: InboxItem, _ change: (inout ItemState) -> Void) {
        var state = states[item.id] ?? ItemState()
        change(&state)
        states[item.id] = state.isEmpty ? nil : state
        persistence.save(states, to: persistence.states)
    }

    // MARK: Accounts

    /// How long the browser has to come back.
    static let signInWait: Duration = .seconds(300)

    /// Signs in through the browser and returns the secrets for `connect`. Only the plugin's declared sign-in page
    /// opens, on one of its hosts. Cancelling the task stops the wait.
    func signIn(pluginID: String, settings: [String: String]) async throws -> [String: String] {
        guard let manifest = PluginRegistry.manifest(pluginID) else { throw PluginError.unknownPlugin(pluginID) }
        guard let signIn = manifest.signIn else { throw PluginError.signIn("no sign-in") }
        let account = privacy.effective(Account(pluginID: pluginID, settings: settings))
        let settings = account.settings
        let http = try guardedClient(for: account)
        let state = OAuth.random(16)
        let loopback = try await Loopback.open(state: state)
        let pkce = OAuth.PKCE()
        let page = try OAuth.authorizeURL(
            signIn, values: settings, redirect: loopback.redirect, pkce: pkce, state: state)
        let host = page.host?.lowercased() ?? ""
        guard GuardedHTTPClient.matches(host, privacy.allowedHosts(for: account, manifest: manifest)) else {
            throw EgressError.blockedHost(host)
        }
        guard environment.open(page) else { throw PluginError.signIn("no browser") }
        let code = try await withThrowingTaskGroup(of: String.self) { group in
            group.addTask { try await loopback.code() }
            group.addTask {
                try await Task.sleep(for: Self.signInWait)
                throw PluginError.signIn("timed out")
            }
            defer { group.cancelAll() }
            do { return try await group.next()! } catch is CancellationError {
                throw PluginError.signIn("cancelled")
            }
        }
        let tokens = try await OAuth.redeem(
            signIn, values: settings, code: code, redirect: loopback.redirect, verifier: pkce.verifier, http: http)
        guard let refresh = tokens.refreshToken else { throw PluginError.signIn("no refresh token") }
        // `signIn` changes with every sign-in, so the account starts over with it (`renewedSecrets`).
        return ["refreshToken": refresh, "signIn": OAuth.random(8)]
    }

    func connect(pluginID: String, name: String?, settings: [String: String], secrets: [String: String]) async throws {
        let name = name?.trimmingCharacters(in: .whitespaces).nilIfEmpty
        let account = Account(pluginID: pluginID, name: name, settings: settings)
        if PluginRegistry.isAssistant(pluginID) {
            let assistant = try assistantPlugin(account, secrets: secrets)
            if preferences.wholeInboxBrief {
                // The first brief doubles as the connection test.
                self.assistant.store(try await assistant.brief(briefItems, now: .now))
            } else {
                // The brief is off: test with one made-up item, so no inbox content leaves the Mac yet.
                _ = try await assistant.digest([Self.connectionTest], topic: "Test", now: .now)
            }
            try accountsService.saveSecrets(secrets, for: account.id)
            accountsService.replaceAssistant(with: account)
            return
        }
        let plugin = try sourcePlugin(account, secrets: secrets, remembering: false)
        var snapshot = try await plugin.fetch()
        let (classifier, fetched) = (classifier, snapshot.items)
        snapshot.items = await Task.detached { fetched.map(classifier.classify) }.value
        // The same person on the same tool and host, connected again: that account takes the new token and
        // keeps its id, so Done, snoozes and pins stay. A new name replaces the old one.
        if let existing = accountsService.existing(sameAs: account, identity: snapshot.identity) {
            snapshot.items = snapshot.items.map { $0.moved(from: account.id, to: existing.id) }
            try accountsService.saveSecrets(secrets, for: existing.id)
            if let name { accountsService.rename(existing.id, to: name) }
            sync.retry(existing.id)
            apply(snapshot, to: existing.id)
            persistence.save(itemsByAccount, to: persistence.cache)
            return
        }
        try accountsService.saveSecrets(secrets, for: account.id)
        accountsService.add(account)
        apply(snapshot, to: account.id)
        persistence.save(itemsByAccount, to: persistence.cache)
    }

    /// What connecting an assistant sends when the whole-inbox brief is off: nothing from the inbox.
    static let connectionTest = InboxItem(
        id: "remora/connection-test", accountID: UUID(), pluginID: "remora", bundle: .reminders,
        title: "Connection test", context: "Remora", date: .now
    )

    /// Keeps an agent's request through the MCP server (V15-01): the most recent `AgentRead.kept`.
    func recordAgentRead(_ read: AgentRead) {
        agentReads = AgentRead.adding(read, to: agentReads)
        persistence.save(agentReads, to: persistence.agentReads)
    }

    /// Removes everything Remora stored on this Mac: accounts, tokens, cache, history, settings.
    /// Everything else is erased even when the Keychain refuses; then that refusal is thrown, so it is said.
    func eraseLocalData() throws {
        undoPoint = nil
        storageIssues = []
        updater.forget()
        let keychain = Result { try accountsService.deleteAllSecrets() }
        environment.notifications.removeAll()
        accountsService.reset()
        itemsByAccount = [:]
        reminders = []
        states = [:]
        sync.reset()
        assistant.reset()
        snoozes.reset()
        learning.reset()
        agentReads = []
        links = [:]
        preferences = Preferences()
        spotlight.reset()
        // Nothing pending or in progress may land after the files are gone.
        writer.discard()
        let left = Self.removeContents(of: environment.folder)
        environment.removeURLCaches()
        environment.images.forget()
        if !left.isEmpty {
            var keychainError: String?
            if case .failure(let error) = keychain { keychainError = error.localizedDescription }
            throw EraseError(files: left, folder: environment.folder.path, keychain: keychainError)
        }
        try keychain.get()
    }

    /// Deletes everything in `folder` and returns the names it couldn't (the folder's own name when it can't be read).
    static func removeContents(of folder: URL) -> [String] {
        let files: [URL]
        do {
            files = try FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: nil)
        } catch CocoaError.fileReadNoSuchFile {
            return []
        } catch {
            return [folder.lastPathComponent]
        }
        return files.compactMap { file in
            do {
                try FileManager.default.removeItem(at: file)
                return nil
            } catch {
                return file.lastPathComponent
            }
        }
    }

    /// The token goes first: when the Keychain refuses, the account stays, with the reason under it.
    func disconnect(_ account: Account) {
        do {
            try accountsService.deleteSecrets(for: account.id)
        } catch {
            sync.setFailure(SourceFailure(kind: .other, message: error.localizedDescription), for: account.id)
            return
        }
        undoPoint = nil
        accountsService.remove(account.id)
        spotlight.remove(account: account.id)
        if PluginRegistry.isAssistant(account.pluginID) { assistant.disconnected() }
        itemsByAccount[account.id] = nil
        sync.forget(account.id)
        environment.notifications.remove(itemsWithPrefix: InboxItem.accountPrefix(account.id))
        persistence.save(itemsByAccount, to: persistence.cache)
        pruneStates()
    }
}

extension InboxModel {
    /// A failed write is said in Settings → Privacy, once per file, and cleared when it works again.
    fileprivate func recordWrite(_ file: String, failure: String?) {
        storageIssues.removeAll { $0.file == file && !$0.corrupt }
        if let failure { storageIssues.append(StorageIssue(file: file, corrupt: false, message: failure)) }
    }

    /// Writes every pending change now and waits for it: when quitting.
    func flushWrites() {
        writer.flush()
    }
}

extension Account {
    /// The same person on the same tool and server: what connecting twice would duplicate.
    func isSame(as other: Account, identity: String) -> Bool {
        guard pluginID == other.pluginID, !identity.isEmpty, self.identity?.lowercased() == identity.lowercased() else {
            return false
        }
        // "gitlab.acme.io", "https://gitlab.acme.io/" and an empty field (the default host) compare by host, and
        // the path: two Azure DevOps organizations share dev.azure.com.
        let host = { (account: Account) -> String? in
            guard let value = account.settings["host"]?.trimmingCharacters(in: .whitespaces), !value.isEmpty,
                let url = URL(string: value.contains("://") ? value : "https://" + value)
            else { return nil }
            var path = url.path
            while path.hasSuffix("/") { path.removeLast() }
            return ((url.host ?? "") + path).lowercased()
        }
        return host(self) == host(other)
    }
}

extension InboxItem {
    /// The item as the account `to` fetched it: its id starts with the new account's (`InboxItem.makeID`).
    func moved(from old: UUID, to new: UUID) -> InboxItem {
        var item = self
        if accountID == old, let local = localID { item.id = Self.makeID(account: new, local: local) }
        item.accountID = new
        return item
    }
}

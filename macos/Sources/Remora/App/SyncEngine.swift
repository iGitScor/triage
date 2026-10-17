import Foundation
import Observation
import RemoraCore
import RemoraPlugins

/// The sync engine: the refresh (polling, the fetches run at once, their results), what each account's last fetch
/// said, which accounts wait before being asked again, whether the Mac is offline, and the notifications held for the
/// next set time. `InboxModel` says what to fetch and stores the items. Same on Windows (`sync.rs`).
@MainActor @Observable
final class SyncEngine {
    /// Each failing account's last failure, by kind.
    private(set) var errors: [UUID: SourceFailure] = [:]
    /// What the last fetch of each account said besides its items: a list cut at its limit, a missing permission.
    private(set) var remarks: [UUID: [String]] = [:]
    /// No network: refreshes wait for it, and the footer says so once instead of every source failing.
    private(set) var isOffline = false
    private(set) var isRefreshing = false
    private(set) var lastRefresh: Date?
    /// Accounts that failed lately, and when to ask them again.
    @ObservationIgnored private var backoff: [UUID: Backoff] = [:]
    /// Accounts fetched since launch: the first fetch announces what arrived while Remora was closed.
    @ObservationIgnored private var synced: Set<UUID> = []
    /// Notifications waiting for the next set time, and when the last batch went out.
    @ObservationIgnored private var waitingNotices: [Notice] = []
    @ObservationIgnored private var batchedAt = Date.now
    @ObservationIgnored private var polling: Task<Void, Never>?

    /// What the footer says about the sources.
    var health: SourcesHealth { SourcesHealth(failures: errors, offline: isOffline) }

    // MARK: Refreshing

    /// One account's fetch: its plugin, or why it couldn't be made.
    struct FetchJob: Sendable {
        let id: UUID
        let plugin: Result<any SourcePlugin, Error>
    }

    /// Refreshes now, then every `interval`. A new call replaces the last.
    func poll(every interval: TimeInterval, refresh: @escaping @MainActor () async -> Void) {
        polling?.cancel()
        polling = Task {
            while !Task.isCancelled {
                await refresh()
                try? await Task.sleep(for: .seconds(interval))
            }
        }
    }

    /// One refresh, unless one is running or the Mac is offline: every tool would fail the same way, so it waits for
    /// the network (`networkChanged`). `jobs` makes the fetches, told which accounts sit this one out; they run at
    /// once, each item classified off the main actor. Fetches run while the main actor is free: a result is kept only
    /// while `keeps` still finds its account, and `apply` stores it. True when one was kept.
    func refresh(
        manual: Bool, interval: TimeInterval, classifier: VerbClassifier,
        jobs: (_ waits: (UUID) -> Bool) -> [FetchJob],
        keeps: (UUID) -> Account?,
        apply: @MainActor (Account, SourceSnapshot) async -> Void
    ) async -> Bool {
        guard !isRefreshing else { return false }
        isRefreshing = true
        defer { isRefreshing = false }
        let now = Date.now
        guard !isOffline else { return false }
        // An account that keeps failing, or is rate-limited, sits this one out and keeps its items and its error.
        let jobs = jobs { waits($0, at: now, manual: manual) }
        var kept = 0
        await withTaskGroup(of: (UUID, Result<SourceSnapshot, Error>).self) { group in
            for job in jobs {
                let (id, made) = (job.id, job.plugin)
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
                // An account erased, disconnected or blocked by the policy meanwhile must not come back (as on
                // Windows). A reconnected account has a new id.
                guard let account = keeps(id) else { continue }
                kept += 1
                switch result {
                case .success(let snapshot):
                    retry(id)
                    await apply(account, snapshot)
                case .failure(let error):
                    failed(id, error: error, interval: interval)
                }
            }
        }
        lastRefresh = .now
        return kept > 0
    }

    /// The network came and went: offline, nothing is refreshed; back online, failures that were only the missing
    /// network go. True when it just came back, so everything is refreshed at once.
    func networkChanged(online: Bool) -> Bool {
        let wasOffline = isOffline
        isOffline = !online
        guard online, wasOffline else { return false }
        errors = errors.filter { $0.value.kind != .offline }
        return true
    }

    /// An account that keeps failing, or is rate-limited, sits a refresh out. `manual`: asked for by the user, which
    /// doesn't wait out a slow-down.
    func waits(_ id: UUID, at now: Date, manual: Bool) -> Bool {
        backoff[id]?.waits(at: now, manual: manual) ?? false
    }

    /// Asked again at once: a fetch that worked, or the same account connected again.
    func retry(_ id: UUID) { backoff[id] = nil }

    /// A fetch that worked: the account's failure ends, its remarks are the new ones. True when it is its first
    /// since launch.
    func succeeded(_ id: UUID, remarks: [String]) -> Bool {
        let first = synced.insert(id).inserted
        errors[id] = nil
        self.remarks[id] = remarks.isEmpty ? nil : remarks
        return first
    }

    /// A failed fetch: the account is asked less often, and a rate limit is waited out. Being offline isn't the
    /// tool's fault: no slow-down for it.
    func failed(_ id: UUID, error: Error, interval: TimeInterval) {
        let failure = SourceFailure(error)
        errors[id] = failure
        if failure.kind == .offline { return }
        var next = backoff[id] ?? Backoff()
        if case HTTPError.rateLimited(let until) = error {
            next.failed(at: .now, interval: interval, rateLimitedUntil: until ?? Date.now.addingTimeInterval(interval))
        } else {
            next.failed(at: .now, interval: interval)
        }
        backoff[id] = next
    }

    /// Why an account isn't used (the policy refuses it, the Keychain refused to delete its token), or nil.
    func setFailure(_ failure: SourceFailure?, for id: UUID) { errors[id] = failure }

    /// Its items were dropped: the next fetch is a first one again.
    func unsynced(_ id: UUID) { synced.remove(id) }

    /// A disconnected account: nothing it said stays.
    func forget(_ id: UUID) {
        errors[id] = nil
        remarks[id] = nil
        synced.remove(id)
    }

    /// Erase.
    func reset() {
        errors = [:]
        remarks = [:]
        synced = []
    }

    // MARK: Notifications

    /// Grouped at set times: what arrived or changed waits for `release`. Returns what goes now.
    func hold(_ notices: [Notice], batching: Bool) -> [Notice] {
        let held = batching ? notices.filter(NotificationBatch.waits) : []
        waitingNotices += held
        return notices.filter { !held.contains($0) }
    }

    /// What waited, at the first set time since the last batch, or at once when grouping was turned off.
    func release(at clock: Date, preferences: Preferences) -> [Notice] {
        let due =
            !preferences.batchNotifications
            || NotificationBatch.isDue(
                NotificationBatch.times(preferences.notificationTimes), since: batchedAt, now: clock)
        guard due else { return [] }
        batchedAt = clock
        let released = NotificationBatch.grouped(waitingNotices)
        waitingNotices = []
        return released
    }
}

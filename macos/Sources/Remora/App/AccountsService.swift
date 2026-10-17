import Foundation
import Observation
import RemoraCore
import RemoraPlugins

/// The accounts service: the connected accounts, their tokens in the Keychain and their sessions. Every change to
/// the list is saved here, so the file and the list never disagree. Same on Windows (`accounts.rs`).
@MainActor @Observable
final class AccountsService {
    private(set) var accounts: [Account]
    /// Per account, in memory: what a refresh may reuse from the last one.
    @ObservationIgnored private var sessions: [UUID: AccountSession] = [:]
    @ObservationIgnored private let secrets: SecretStore
    @ObservationIgnored private let persistence: Persistence

    init(accounts: [Account], secrets: SecretStore, persistence: Persistence) {
        self.accounts = accounts
        self.secrets = secrets
        self.persistence = persistence
    }

    func account(_ id: UUID) -> Account? { accounts.first { $0.id == id } }

    var sourceAccounts: [Account] { accounts.filter { !PluginRegistry.isAssistant($0.pluginID) } }

    /// The same person on the same tool and host, already connected: connecting again updates that one.
    func existing(sameAs account: Account, identity: String) -> Account? {
        accounts.first { $0.isSame(as: account, identity: identity) }
    }

    // MARK: The list, saved on every change

    func rename(_ id: UUID, to name: String?) {
        guard let index = accounts.firstIndex(where: { $0.id == id }) else { return }
        accounts[index].name = name
        save()
    }

    func add(_ account: Account) {
        accounts.append(account)
        save()
    }

    /// One assistant at a time: a new one replaces the previous one.
    func replaceAssistant(with account: Account) {
        accounts.removeAll { PluginRegistry.isAssistant($0.pluginID) }
        add(account)
    }

    /// Who the token belongs to, as the last fetch said.
    func setIdentity(_ identity: String, for id: UUID) {
        guard let index = accounts.firstIndex(where: { $0.id == id }), accounts[index].identity != identity else {
            return
        }
        accounts[index].identity = identity
        save()
    }

    /// The account and what its session kept.
    func remove(_ id: UUID) {
        accounts.removeAll { $0.id == id }
        sessions[id] = nil
        save()
    }

    /// Erase: nothing is left, and nothing is saved (the files go).
    func reset() {
        accounts = []
        sessions = [:]
    }

    private func save() {
        persistence.save(accounts, to: persistence.accounts)
    }

    // MARK: Sessions

    /// What a refresh of this account may reuse: a new session when its settings or token changed.
    func session(for account: Account, secrets: [String: String]) -> AccountSession {
        let fingerprint = AccountSession.fingerprint(settings: account.settings, secrets: secrets)
        if let session = sessions[account.id], session.fingerprint == fingerprint { return session }
        let session = AccountSession(fingerprint: fingerprint)
        sessions[account.id] = session
        return session
    }

    // MARK: Tokens

    func secrets(for id: UUID) -> [String: String] { secrets.secrets(for: id) }

    func saveSecrets(_ values: [String: String], for id: UUID) throws { try secrets.save(values, for: id) }

    func deleteSecrets(for id: UUID) throws { try secrets.delete(id) }

    func deleteAllSecrets() throws { try secrets.deleteAll() }

    /// Secrets the account's plugin renewed during the fetch (a Microsoft sign-in's refresh token) go to the
    /// Keychain, as a connection's do. A failed save keeps the old one, still valid.
    func saveRenewed(for account: Account) async {
        guard let session = sessions[account.id] else { return }
        let renewed = await session.cache.takeRenewed()
        guard !renewed.isEmpty, self.account(account.id) != nil else { return }
        // The old one stays valid for a while, and a Keychain that refuses shows at the next connection.
        // Nothing read (a Keychain that refuses) must not become an entry holding only the renewed secret.
        let current = secrets.secrets(for: account.id)
        guard !current.isEmpty else { return }
        try? secrets.save(current.merging(renewed) { _, new in new }, for: account.id)
    }
}

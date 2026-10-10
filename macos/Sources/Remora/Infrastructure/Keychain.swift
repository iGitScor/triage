import Foundation
import RemoraCore
import Security

/// Where the accounts' tokens are kept: the Keychain in the app, memory in tests.
@MainActor
protocol SecretStore: AnyObject {
    func secrets(for account: UUID) -> [String: String]
    func save(_ secrets: [String: String], for account: UUID) throws
    func delete(_ account: UUID) throws
    /// Erase: every secret Remora ever stored.
    func deleteAll() throws
}

/// Why a token couldn't be saved or removed: a refusal must never look like a success.
enum KeychainError: LocalizedError, Equatable {
    /// The vault exists but couldn't be read or decoded: writing would replace every account's tokens.
    case unreadable
    case status(OSStatus)

    var errorDescription: String? {
        switch self {
        case .unreadable:
            L(
                "Remora can’t read its Keychain item, so it leaves it unchanged. Quit and reopen Remora, or use Erase local data in Settings → Privacy to start again."
            )
        case .status(errSecUserCanceled), .status(errSecAuthFailed), .status(errSecInteractionNotAllowed):
            L("Keychain access was refused. Try again and choose Always Allow.")
        case .status(let status):
            L("The Keychain refused the change (error %@).", String(status))
        }
    }
}

/// The generic-password items the vault reads and writes: the system Keychain in the app, a dictionary in tests.
/// `read` returns nil only when there is no item; every other failure throws.
protocol KeychainItems {
    func read(service: String, account: String) throws -> Data?
    func write(_ data: Data, service: String, account: String) throws
    func delete(service: String, account: String) throws
    /// Every item of the service, whatever its account.
    func deleteAll(service: String) throws
}

/// All accounts' secrets in a single keychain item, read once per launch.
/// One item means at most one access prompt when macOS doesn't recognize a new build.
@MainActor
final class Keychain: SecretStore {
    static let live = Keychain(items: SystemKeychain())

    private static let service = "fr.igitscor.remora"
    private static let vaultAccount = "secrets"
    /// Earlier layouts stored one item per account; they are merged into the vault on first read.
    /// Names used before the app became Remora; their items are moved here on first read.
    private static let legacyVaultServices = ["fr.igitscor.perch"]
    private static let legacyServices = ["fr.igitscor.remora", "fr.igitscor.perch"]

    private let items: KeychainItems
    private var vault: [String: [String: String]]?
    /// The vault couldn't be read this launch: it is never written over, only erased.
    private var unreadable = false

    init(items: KeychainItems) {
        self.items = items
    }

    func secrets(for account: UUID) -> [String: String] {
        guard let vault = try? loadVault() else { return [:] }
        if let secrets = vault[account.uuidString] { return secrets }
        guard let legacy = legacySecrets(for: account) else { return [:] }
        // The old item goes only once the vault holds its copy.
        if (try? save(legacy, for: account)) != nil { try? deleteLegacy(account) }
        return legacy
    }

    func save(_ secrets: [String: String], for account: UUID) throws {
        var vault = try loadVault()
        vault[account.uuidString] = secrets
        try write(vault)
    }

    func delete(_ account: UUID) throws {
        var vault = try loadVault()
        vault[account.uuidString] = nil
        try write(vault)
        try deleteLegacy(account)
    }

    /// Erase: every item Remora ever stored, the vault and the per-account items of earlier versions (Perch
    /// included), whatever account they belong to. An unreadable vault goes too.
    func deleteAll() throws {
        for service in Set([Self.service] + Self.legacyVaultServices + Self.legacyServices) {
            try items.deleteAll(service: service)
        }
        vault = [:]
        unreadable = false
    }

    private func loadVault() throws -> [String: [String: String]] {
        if let vault { return vault }
        if unreadable { throw KeychainError.unreadable }
        do {
            if let data = try items.read(service: Self.service, account: Self.vaultAccount) {
                let loaded = try JSONDecoder().decode([String: [String: String]].self, from: data)
                vault = loaded
                return loaded
            }
        } catch {
            unreadable = true
            throw KeychainError.unreadable
        }
        for legacy in Self.legacyVaultServices {
            guard let data = try? items.read(service: legacy, account: Self.vaultAccount),
                let loaded = try? JSONDecoder().decode([String: [String: String]].self, from: data)
            else { continue }
            try write(loaded)
            try? items.delete(service: legacy, account: Self.vaultAccount)
            return loaded
        }
        vault = [:]
        return [:]
    }

    /// The cached vault changes only once the Keychain has accepted the new one.
    private func write(_ secrets: [String: [String: String]]) throws {
        try items.write(try JSONEncoder().encode(secrets), service: Self.service, account: Self.vaultAccount)
        vault = secrets
    }

    private func legacySecrets(for account: UUID) -> [String: String]? {
        for service in Self.legacyServices {
            if let data = try? items.read(service: service, account: account.uuidString),
                let secrets = try? JSONDecoder().decode([String: String].self, from: data)
            {
                return secrets
            }
        }
        return nil
    }

    private func deleteLegacy(_ account: UUID) throws {
        for service in Self.legacyServices {
            try items.delete(service: service, account: account.uuidString)
        }
    }
}

/// The login keychain's generic passwords.
struct SystemKeychain: KeychainItems {
    func read(service: String, account: String) throws -> Data? {
        var request = query(service: service, account: account)
        request[kSecReturnData as String] = true
        request[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: AnyObject?
        let status = SecItemCopyMatching(request as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess else { throw KeychainError.status(status) }
        return result as? Data
    }

    func write(_ data: Data, service: String, account: String) throws {
        let attributes = [kSecValueData as String: data]
        let base = query(service: service, account: account)
        var status = SecItemUpdate(base as CFDictionary, attributes as CFDictionary)
        if status == errSecItemNotFound {
            status = SecItemAdd(base.merging(attributes) { $1 } as CFDictionary, nil)
        }
        guard status == errSecSuccess else { throw KeychainError.status(status) }
    }

    func delete(service: String, account: String) throws {
        let status = SecItemDelete(query(service: service, account: account) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else { throw KeychainError.status(status) }
    }

    func deleteAll(service: String) throws {
        let all: [String: Any] = [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service]
        // The file-based keychain may delete one match per call: repeat until none is left.
        for _ in 0..<100 {
            let status = SecItemDelete(all as CFDictionary)
            if status == errSecItemNotFound { return }
            guard status == errSecSuccess else { throw KeychainError.status(status) }
        }
    }

    private func query(service: String, account: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
    }
}

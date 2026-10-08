import Foundation
import Security

/// All accounts' secrets in a single keychain item, read once per launch.
/// One item means at most one access prompt when macOS doesn't recognize a new build.
@MainActor
enum Keychain {
    private static let service = "fr.igitscor.remora"
    private static let vaultAccount = "secrets"
    /// Earlier layouts stored one item per account; they are merged into the vault on first read.
    /// Names used before the app became Remora; their items are moved here on first read.
    private static let legacyVaultServices = ["fr.igitscor.perch"]
    private static let legacyServices = ["fr.igitscor.remora", "fr.igitscor.perch"]

    private static var vault: [String: [String: String]]?

    static func secrets(for account: UUID) -> [String: String] {
        if let secrets = loadVault()[account.uuidString] { return secrets }
        guard let legacy = legacySecrets(for: account) else { return [:] }
        save(legacy, for: account)
        deleteLegacy(account)
        return legacy
    }

    static func save(_ secrets: [String: String], for account: UUID) {
        var vault = loadVault()
        vault[account.uuidString] = secrets
        write(vault)
    }

    static func delete(_ account: UUID) {
        var vault = loadVault()
        vault[account.uuidString] = nil
        write(vault)
        deleteLegacy(account)
    }

    private static func loadVault() -> [String: [String: String]] {
        if let vault { return vault }
        if let data = read(service: service, account: vaultAccount) {
            let loaded = (try? JSONDecoder().decode([String: [String: String]].self, from: data)) ?? [:]
            vault = loaded
            return loaded
        }
        for legacy in legacyVaultServices {
            guard let data = read(service: legacy, account: vaultAccount),
                  let loaded = try? JSONDecoder().decode([String: [String: String]].self, from: data) else { continue }
            write(loaded)
            SecItemDelete(query(service: legacy, account: vaultAccount) as CFDictionary)
            return loaded
        }
        vault = [:]
        return [:]
    }

    private static func write(_ secrets: [String: [String: String]]) {
        vault = secrets
        let data = (try? JSONEncoder().encode(secrets)) ?? Data()
        let attributes = [kSecValueData as String: data]
        let base = query(service: service, account: vaultAccount)
        if SecItemUpdate(base as CFDictionary, attributes as CFDictionary) == errSecItemNotFound {
            SecItemAdd(base.merging(attributes) { $1 } as CFDictionary, nil)
        }
    }

    private static func legacySecrets(for account: UUID) -> [String: String]? {
        for service in legacyServices {
            if let data = read(service: service, account: account.uuidString),
               let secrets = try? JSONDecoder().decode([String: String].self, from: data) {
                return secrets
            }
        }
        return nil
    }

    private static func deleteLegacy(_ account: UUID) {
        for service in legacyServices {
            SecItemDelete(query(service: service, account: account.uuidString) as CFDictionary)
        }
    }

    private static func read(service: String, account: String) -> Data? {
        var request = query(service: service, account: account)
        request[kSecReturnData as String] = true
        request[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: AnyObject?
        guard SecItemCopyMatching(request as CFDictionary, &result) == errSecSuccess else { return nil }
        return result as? Data
    }

    private static func query(service: String, account: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
    }
}

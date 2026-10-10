import Foundation
import Testing
import RemoraCore
@testable import Remora

struct JSONStoreTests {
    @Test func savesAndLoadsInItsFolder() throws {
        let folder = temporaryFolder()
        let store = JSONStore<[String: Int]>("counts", in: folder)
        #expect(store.load() == nil, "nothing saved yet")
        try store.save(["a": 1])
        #expect(store.url == folder.appending(path: "counts.json"))
        #expect(JSONStore<[String: Int]>("counts", in: folder).load() == ["a": 1])
    }

    /// A file that doesn't decode is kept aside, never overwritten, and said.
    @Test func aCorruptFileIsKeptAside() throws {
        let folder = temporaryFolder()
        let store = JSONStore<[String]>("accounts", in: folder)
        try Data("{half a file".utf8).write(to: store.url)
        #expect(throws: JSONStore<[String]>.Failure.corrupt(name: "accounts.json", aside: "accounts.json.corrupt")) { try store.read() }
        #expect(try Data(contentsOf: folder.appending(path: "accounts.json.corrupt")) == Data("{half a file".utf8))
        #expect(try store.read() == nil, "then nothing saved")
        try Data("again".utf8).write(to: store.url)
        #expect(throws: JSONStore<[String]>.Failure.corrupt(name: "accounts.json", aside: "accounts.json.corrupt2")) { try store.read() }
    }

    /// A write that fails is an error, not silence.
    @Test func aFailedWriteThrows() throws {
        let folder = temporaryFolder().appending(path: "missing/deeper")
        #expect(throws: JSONStore<[String]>.Failure.self) { try JSONStore<[String]>("accounts", in: folder).save(["a"]) }
    }

    /// Only you can read the files, and what Remora can fetch again stays out of backups.
    @Test func filesAreYoursOnlyAndCachesStayOutOfBackups() throws {
        let folder = temporaryFolder()
        let cache = JSONStore<[String]>("cache", in: folder, backedUp: false)
        let settings = JSONStore<[String]>("preferences", in: folder)
        for _ in 0..<2 {
            try cache.save(["message"])
            try settings.save(["setting"])
        }
        for store in [cache.url, settings.url] {
            let permissions = try FileManager.default.attributesOfItem(atPath: store.path)[.posixPermissions] as? Int
            #expect(permissions == 0o600, "\(store.lastPathComponent)")
        }
        #expect(try cache.url.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup == true)
        #expect(try settings.url.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup == false)
    }
}

struct PreferencesTests {
    @Test func missingKeysKeepTheirDefaults() throws {
        let saved = Data(#"{"refreshMinutes": 10, "allowExternalAI": true}"#.utf8)
        let preferences = try JSONDecoder().decode(Preferences.self, from: saved)
        #expect(preferences.refreshMinutes == 10 && preferences.allowExternalAI)
        var expected = Preferences()
        expected.refreshMinutes = 10
        expected.allowExternalAI = true
        #expect(preferences == expected, "a new preference never resets the others")
    }

    @Test func roundTrips() throws {
        var preferences = Preferences()
        preferences.hiddenContentPlugins = ["slack"]
        preferences.assistantExcludedSources = ["reminders"]
        preferences.menuBarCount = .everything
        let data = try JSONEncoder().encode(preferences)
        #expect(try JSONDecoder().decode(Preferences.self, from: data) == preferences)
    }
}

struct ManagedPolicyTests {
    @Test func theUsersChoicesApplyWhenNothingIsManaged() {
        var preferences = Preferences()
        preferences.allowedPlugins = ["github"]
        preferences.allowExternalAI = true
        let policy = ManagedPolicy.policy(user: preferences, managed: ManagedDictionary())
        #expect(policy.allowedPlugins == ["github"] && policy.allowExternalAI && policy.allowRemoteImages)
        #expect(!ManagedPolicy.isManaged(ManagedDictionary()))
    }

    @Test func theOrganizationWins() {
        var preferences = Preferences()
        preferences.allowExternalAI = true
        let managed = ManagedDictionary(values: [
            "AllowedPlugins": ["slack"], "AllowExternalAI": false, "AllowRemoteImages": NSNumber(value: false),
        ])
        let policy = ManagedPolicy.policy(user: preferences, managed: managed)
        #expect(policy.allowedPlugins == ["slack"] && !policy.allowExternalAI && !policy.allowRemoteImages)
        #expect(ManagedPolicy.isManaged(managed))
    }

    /// A value of the wrong type allows nothing, as before (the same rule as on Windows).
    @Test func aValueOfTheWrongTypeAllowsNothing() {
        let managed = ManagedDictionary(values: ["AllowedPlugins": "github", "AllowExternalAI": [true]])
        var preferences = Preferences()
        preferences.allowExternalAI = true
        let policy = ManagedPolicy.policy(user: preferences, managed: managed)
        #expect(policy.allowedPlugins == [] && !policy.allowExternalAI)
    }

    @Test func booleansReadAsUserDefaultsDoes() {
        for (value, expected) in [("YES", true), ("true", true), ("1", true), ("no", false), ("0", false)] {
            let policy = ManagedPolicy.policy(user: Preferences(), managed: ManagedDictionary(values: ["AllowExternalAI": value]))
            #expect(policy.allowExternalAI == expected, "\(value)")
        }
    }

    /// The settings an organization can force, and what's ignored.
    @Test func forcedSettingsWin() {
        var preferences = Preferences()
        preferences.hiddenContentPlugins = ["linear"]
        preferences.refreshMinutes = 5
        preferences.openInApps = true
        let managed = ManagedDictionary(values: [
            "HiddenContentSources": ["slack"], "RefreshMinutes": 15, "OpenInApps": false, "ClaudeCodePath": "/opt/claude/bin/claude",
        ])
        let effective = ManagedPolicy.effective(preferences, managed: managed)
        #expect(effective.hiddenContentPlugins == ["linear", "slack"], "the organization only adds")
        #expect(effective.refreshMinutes == 15 && !effective.openInApps)
        #expect(ManagedPolicy.claudeCodePath(managed) == "/opt/claude/bin/claude")
        #expect(ManagedPolicy.isForced("RefreshMinutes", managed) && !ManagedPolicy.isForced("RefreshMinutes", ManagedDictionary()))
        #expect(!ManagedPolicy.isManaged(ManagedDictionary(values: ["RefreshMinutes": 15])), "the Privacy pane stays editable")

        let odd = ManagedDictionary(values: ["RefreshMinutes": 0, "ClaudeCodePath": "  ", "HiddenContentSources": "slack"])
        let kept = ManagedPolicy.effective(preferences, managed: odd)
        #expect(kept.refreshMinutes == 5, "out of range: ignored")
        #expect(ManagedPolicy.claudeCodePath(odd) == nil)
        #expect(kept.hiddenContentPlugins == ["linear"])
        #expect(ManagedPolicy.effective(preferences, managed: ManagedDictionary(values: ["RefreshMinutes": "30"])).refreshMinutes == 30)
    }

    @Test func assistantExclusionsAddUpAndModelsComeFromTheOrganization() {
        var preferences = Preferences()
        preferences.assistantExcludedSources = ["slack"]
        let managed = ManagedDictionary(values: ["AIExcludedSources": ["reminders"], "AllowedAIModels": ["claude-haiku-5-5"]])
        let policy = ManagedPolicy.assistantPolicy(user: preferences, managed: managed)
        #expect(policy.excludedSources == ["slack", "reminders"])
        #expect(policy.allowedModels == ["claude-haiku-5-5"])
        #expect(ManagedPolicy.assistantPolicy(user: preferences, managed: ManagedDictionary()).allowedModels == nil)
    }
}

@MainActor
struct KeychainTests {
    let account = UUID()

    @Test func keepsEveryAccountInOneItem() throws {
        let items = MemoryKeychainItems()
        let keychain = Keychain(items: items)
        try keychain.save(["token": "a"], for: account)
        try keychain.save(["token": "b"], for: UUID())
        #expect(items.items["fr.igitscor.remora"]?.keys.sorted() == ["secrets"], "one item, one access prompt")
        #expect(Keychain(items: items).secrets(for: account) == ["token": "a"], "read back by a new launch")
        try keychain.delete(account)
        #expect(Keychain(items: items).secrets(for: account) == [:])
    }

    /// Earlier versions kept one item per account: it moves into the vault on first read.
    @Test func movesAPerAccountItemIntoTheVault() throws {
        let items = MemoryKeychainItems()
        try items.write(try JSONEncoder().encode(["token": "old"]), service: "fr.igitscor.perch", account: account.uuidString)
        let keychain = Keychain(items: items)
        #expect(keychain.secrets(for: account) == ["token": "old"])
        #expect(try items.read(service: "fr.igitscor.perch", account: account.uuidString) == nil, "the old item is gone")
        #expect(Keychain(items: items).secrets(for: account) == ["token": "old"])
    }

    @Test func movesThePerchVault() throws {
        let items = MemoryKeychainItems()
        try items.write(try JSONEncoder().encode([account.uuidString: ["token": "perch"]]), service: "fr.igitscor.perch", account: "secrets")
        #expect(Keychain(items: items).secrets(for: account) == ["token": "perch"])
        #expect(items.items["fr.igitscor.perch"]?.isEmpty ?? true)
        #expect(try items.read(service: "fr.igitscor.remora", account: "secrets") != nil)
    }

    /// Erase removes every item of every name the app ever used, whatever account it belongs to.
    @Test func eraseRemovesEveryService() throws {
        let items = MemoryKeychainItems()
        try items.write(Data("x".utf8), service: "fr.igitscor.perch", account: "someone")
        try items.write(Data("y".utf8), service: "fr.igitscor.remora", account: UUID().uuidString)
        let keychain = Keychain(items: items)
        try keychain.save(["token": "a"], for: account)
        try keychain.deleteAll()
        #expect(items.items.values.allSatisfy { $0.isEmpty })
        #expect(keychain.secrets(for: account) == [:])
    }

    /// A refused write is an error, and the vault in memory keeps what the Keychain really holds.
    @Test func aRefusedWriteThrows() throws {
        let items = MemoryKeychainItems()
        let keychain = Keychain(items: items)
        try keychain.save(["token": "a"], for: account)
        items.refusing = true
        #expect(throws: KeychainError.self) { try keychain.save(["token": "b"], for: UUID()) }
        #expect(throws: KeychainError.self) { try keychain.delete(account) }
        #expect(keychain.secrets(for: account) == ["token": "a"])
    }

    /// A vault that can't be decoded is never written over, so no account loses its token. Erase clears it.
    @Test func anUnreadableVaultIsNeverOverwritten() throws {
        let items = MemoryKeychainItems()
        try items.write(Data("garbage".utf8), service: "fr.igitscor.remora", account: "secrets")
        let keychain = Keychain(items: items)
        #expect(keychain.secrets(for: account) == [:])
        #expect(throws: KeychainError.unreadable) { try keychain.save(["token": "a"], for: account) }
        #expect(throws: KeychainError.unreadable) { try keychain.delete(account) }
        #expect(try items.read(service: "fr.igitscor.remora", account: "secrets") == Data("garbage".utf8))
        try keychain.deleteAll()
        try keychain.save(["token": "a"], for: account)
        #expect(Keychain(items: items).secrets(for: account) == ["token": "a"])
    }
}

/// A reviewer's state is said in words, not only shown by a ring's colour.
struct ReviewerDescriptionTests {
    @Test func eachStateIsSpoken() {
        #expect(PeopleStack.describe(Person(name: "erin", tone: .accent)) == "erin, approved")
        #expect(PeopleStack.describe(Person(name: "dave", tone: .negative)) == "dave, changes requested")
        #expect(PeopleStack.describe(Person(name: "frank")) == "frank, waiting")
    }
}

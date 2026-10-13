import Foundation
import RemoraCore

/// What one account keeps between refreshes, in memory: what its plugin remembers (who the token belongs to) and the
/// answers its conditional requests can reuse. A new one starts whenever the account's settings or token change, so
/// nothing learned with an old token or host is reused.
struct AccountSession {
    let fingerprint: Int
    let cache = MemoryAccountCache()
    let etags = ETagStore()

    /// Settings and secrets, hashed for this run only (Swift seeds `Hasher` per process): never stored.
    static func fingerprint(settings: [String: String], secrets: [String: String]) -> Int {
        var hasher = Hasher()
        // A secret the plugin renews on its own is left out, or every renewal would start the account over.
        let secrets = secrets.filter { !renewedSecrets.contains($0.key) }
        for (key, value) in settings.merging(secrets, uniquingKeysWith: { _, secret in secret }).sorted(by: {
            $0.key < $1.key
        }) {
            hasher.combine(key)
            hasher.combine(value)
        }
        return hasher.finalize()
    }
}

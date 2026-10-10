import CryptoKit
import Foundation

/// A release number ("0.3.2"), compared part by part: 0.10 is newer than 0.9.
public struct AppVersion: Comparable, CustomStringConvertible, Sendable {
    public let parts: [Int]

    public init?(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespaces)
        let core = trimmed.hasPrefix("v") ? String(trimmed.dropFirst()) : trimmed
        let parts = core.split(separator: ".", omittingEmptySubsequences: false).map { Int($0) }
        guard !parts.isEmpty, parts.count <= 4, parts.allSatisfy({ ($0 ?? -1) >= 0 }) else { return nil }
        self.parts = parts.compactMap { $0 }
    }

    public var description: String { parts.map(String.init).joined(separator: ".") }

    public static func < (lhs: AppVersion, rhs: AppVersion) -> Bool {
        for index in 0..<max(lhs.parts.count, rhs.parts.count) {
            let left = index < lhs.parts.count ? lhs.parts[index] : 0
            let right = index < rhs.parts.count ? rhs.parts[index] : 0
            if left != right { return left < right }
        }
        return false
    }

    public static func == (lhs: AppVersion, rhs: AppVersion) -> Bool { !(lhs < rhs) && !(rhs < lhs) }
}

/// `latest.json`, published with each release. The Windows app reads the same file with Tauri's updater,
/// so it keeps Tauri's shape; the Mac has its own platform key, signed with its own Ed25519 key.
public struct UpdateManifest: Decodable, Sendable {
    public struct Platform: Decodable, Sendable {
        public let url: URL
        /// Base64 Ed25519 signature of the downloaded file's bytes.
        public let signature: String
    }

    public let version: String
    public let notes: String?
    public let platforms: [String: Platform]

    public static let macPlatform = "macos-universal"
}

/// A newer release this Mac can install.
public struct UpdateOffer: Equatable, Sendable {
    public let version: AppVersion
    public let url: URL
    public let signature: String
    public let notes: String?

    /// Nil when the manifest has no Mac file, isn't newer, or points anywhere but a GitHub https download.
    public static func from(_ manifest: UpdateManifest, current: AppVersion) -> UpdateOffer? {
        guard let version = AppVersion(manifest.version), version > current,
            let mac = manifest.platforms[UpdateManifest.macPlatform],
            mac.url.scheme == "https", UpdateHosts.allows(mac.url)
        else { return nil }
        return UpdateOffer(version: version, url: mac.url, signature: mac.signature, notes: manifest.notes)
    }
}

/// Where updates come from: the release's files on GitHub, which redirect to GitHub's download servers. Nothing else,
/// and no token is ever sent there.
public enum UpdateHosts {
    public static let manifest = URL(string: "https://github.com/iGitScor/triage/releases/latest/download/latest.json")!
    public static let hosts = ["github.com", "githubusercontent.com"]

    public static func allows(_ url: URL?) -> Bool {
        guard let url, url.scheme?.lowercased() == "https", let host = url.host?.lowercased() else { return false }
        return GuardedHTTPClient.matches(host, hosts)
    }
}

/// The Ed25519 check that a download is the one the release workflow signed.
public enum UpdateSignature {
    /// `publicKey` is the base64 raw 32-byte key from the app's Info.plist (`RemoraUpdatePublicKey`).
    public static func isValid(_ data: Data, signature: String, publicKey: String) -> Bool {
        guard let keyData = Data(base64Encoded: publicKey),
            let key = try? Curve25519.Signing.PublicKey(rawRepresentation: keyData),
            let signatureData = Data(base64Encoded: signature.trimmingCharacters(in: .whitespacesAndNewlines))
        else { return false }
        return key.isValidSignature(signatureData, for: data)
    }
}

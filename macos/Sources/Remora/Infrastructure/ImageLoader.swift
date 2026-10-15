import AppKit
import RemoraCore

/// Avatars and a workspace's own emoji. They load through their own session, which keeps nothing on disk (no URL
/// cache, no cookies, no credentials) and follows a redirect only on the same host, like the API client; decoded
/// images stay in memory until Erase or quit. Same rules on Windows (`avatars.rs`).
@MainActor
final class ImageLoader {
    /// Images kept; the cache starts again when full.
    static let capacity = 300
    /// Avatars are a few kilobytes: anything near this is not one.
    nonisolated static let maxBytes = 1_000_000

    private let session: URLSession
    private var images: [URL: NSImage] = [:]
    private var loading: [URL: Task<Data?, Never>] = [:]
    /// Bumped by `forget`, so a load in progress then doesn't land afterwards.
    private var generation = 0

    init(session: URLSession = ImageLoader.makeSession()) {
        self.session = session
    }

    /// `configuration` is ephemeral; tests add a stand-in protocol to it.
    nonisolated static func makeSession(_ configuration: URLSessionConfiguration = .ephemeral) -> URLSession {
        configuration.urlCache = nil
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        configuration.httpCookieStorage = nil
        configuration.httpCookieAcceptPolicy = .never
        configuration.httpShouldSetCookies = false
        configuration.urlCredentialStorage = nil
        return URLSession(configuration: configuration, delegate: SameHostRedirects(), delegateQueue: nil)
    }

    /// The image already loaded, if any.
    func cached(_ url: URL) -> NSImage? { images[url] }

    /// The image at `url`, loaded once; nil when it isn't an image or can't be loaded. Callers check the allowlist.
    func image(at url: URL) async -> NSImage? {
        if let image = images[url] { return image }
        let generation = generation
        let task = loading[url] ?? Task { [session] in await Self.fetch(url, through: session) }
        loading[url] = task
        let data = await task.value
        guard generation == self.generation else { return nil }
        loading[url] = nil
        guard let data, let image = NSImage(data: data) else { return nil }
        if images.count >= Self.capacity { images.removeAll() }
        images[url] = image
        return image
    }

    /// Erase: no image kept from before.
    func forget() {
        generation += 1
        for task in loading.values { task.cancel() }
        loading = [:]
        images = [:]
    }

    /// A 2xx `image/*` answer of at most `maxBytes`, without credentials.
    nonisolated private static func fetch(_ url: URL, through session: URLSession) async -> Data? {
        guard url.scheme?.lowercased() == "https",
            let (bytes, response) = try? await session.bytes(for: .get(url, timeout: 15)),
            let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode),
            http.mimeType?.lowercased().hasPrefix("image/") == true, http.expectedContentLength <= maxBytes
        else { return nil }
        var data = Data()
        do {
            for try await byte in bytes {
                data.append(byte)
                if data.count > maxBytes { return nil }
            }
        } catch {
            return nil
        }
        return data
    }
}

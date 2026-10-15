import AppKit
import Foundation
import Testing

@testable import Remora
@testable import RemoraCore

/// A stand-in image host: answers from `routes` and records what was asked, so no test reaches the network.
final class ImageServer: URLProtocol, @unchecked Sendable {
    enum Route {
        case image(Data, headers: [String: String] = [:])
        case redirect(URL)
        case page
    }

    private static let lock = NSLock()
    nonisolated(unsafe) private static var routes: [URL: Route] = [:]
    nonisolated(unsafe) private static var asked: [URL] = []

    static func session() -> URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [ImageServer.self]
        return ImageLoader.makeSession(configuration)
    }

    static func serve(_ route: Route, at url: URL) {
        lock.withLock { routes[url] = route }
    }

    /// How many times `url` was asked for.
    static func requests(to url: URL) -> Int {
        lock.withLock { asked.filter { $0 == url }.count }
    }

    /// A 2×2 PNG.
    static let png: Data = {
        let bitmap = NSBitmapImageRep(
            bitmapDataPlanes: nil, pixelsWide: 2, pixelsHigh: 2, bitsPerSample: 8, samplesPerPixel: 4,
            hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        return bitmap.representation(using: .png, properties: [:])!
    }()

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let url = request.url else { return }
        let route = Self.lock.withLock {
            Self.asked.append(url)
            return Self.routes[url]
        }
        switch route {
        case .image(let data, let headers)?:
            answer(200, ["Content-Type": "image/png"].merging(headers) { $1 }, data)
        case .redirect(let target)?:
            let response = HTTPURLResponse(
                url: url, statusCode: 302, httpVersion: "HTTP/1.1", headerFields: ["Location": target.absoluteString])!
            client?.urlProtocol(self, wasRedirectedTo: URLRequest(url: target), redirectResponse: response)
            answer(302, ["Location": target.absoluteString], Data())
        case .page?:
            answer(200, ["Content-Type": "text/html"], Data("<html></html>".utf8))
        case nil:
            answer(404, [:], Data())
        }
    }

    override func stopLoading() {}

    private func answer(_ status: Int, _ headers: [String: String], _ body: Data) {
        let response = HTTPURLResponse(
            url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: headers)!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .allowed)
        client?.urlProtocol(self, didLoad: body)
        client?.urlProtocolDidFinishLoading(self)
    }
}

/// Avatars load through their own session: nothing on disk, redirects kept on the host, the allowlist first.
@MainActor
struct ImageLoaderTests {
    private func url(_ text: String) -> URL { URL(string: text)! }

    @Test func theSessionKeepsNothingOnDisk() {
        let configuration = ImageLoader.makeSession().configuration
        #expect(configuration.urlCache == nil)
        #expect(configuration.httpCookieStorage == nil)
        #expect(configuration.urlCredentialStorage == nil)
        #expect(!configuration.httpShouldSetCookies)
        #expect(configuration.requestCachePolicy == .reloadIgnoringLocalCacheData)
    }

    @Test func anImageIsLoadedOnceThenKeptInMemoryOnly() async {
        let avatar = url("https://avatars.example.com/once.png")
        ImageServer.serve(
            .image(ImageServer.png, headers: ["Cache-Control": "max-age=86400", "Set-Cookie": "seen=1"]), at: avatar)
        let session = ImageServer.session()
        let loader = ImageLoader(session: session)
        #expect(await loader.image(at: avatar) != nil)
        #expect(await loader.image(at: avatar) != nil)
        #expect(ImageServer.requests(to: avatar) == 1, "the second time from memory")
        #expect(loader.cached(avatar) != nil)

        // Another loader on the same session asks again: nothing went to a URL cache.
        #expect(await ImageLoader(session: session).image(at: avatar) != nil)
        #expect(ImageServer.requests(to: avatar) == 2)

        loader.forget()
        #expect(loader.cached(avatar) == nil, "Erase forgets them")
    }

    @Test func aRedirectOffTheHostIsRefused() async {
        let avatar = url("https://avatars.example.com/bounce.png")
        let elsewhere = url("https://tracker.example.net/seen.png")
        ImageServer.serve(.redirect(elsewhere), at: avatar)
        ImageServer.serve(.image(ImageServer.png), at: elsewhere)
        #expect(await ImageLoader(session: ImageServer.session()).image(at: avatar) == nil)
        #expect(ImageServer.requests(to: elsewhere) == 0)
    }

    @Test func aRedirectOnTheSameHostIsFollowed() async {
        let avatar = url("https://avatars.example.com/moved.png")
        let moved = url("https://avatars.example.com/here.png")
        ImageServer.serve(.redirect(moved), at: avatar)
        ImageServer.serve(.image(ImageServer.png), at: moved)
        #expect(await ImageLoader(session: ImageServer.session()).image(at: avatar) != nil)
        #expect(ImageServer.requests(to: moved) == 1)
    }

    @Test func onlyImagesOverHTTPSLoad() async {
        let page = url("https://avatars.example.com/page")
        ImageServer.serve(.page, at: page)
        let plain = url("http://avatars.example.com/plain.png")
        ImageServer.serve(.image(ImageServer.png), at: plain)
        let big = url("https://avatars.example.com/big.png")
        ImageServer.serve(.image(Data(count: ImageLoader.maxBytes + 1)), at: big)
        let loader = ImageLoader(session: ImageServer.session())
        #expect(await loader.image(at: page) == nil)
        #expect(await loader.image(at: plain) == nil)
        #expect(ImageServer.requests(to: plain) == 0)
        #expect(await loader.image(at: big) == nil)
    }

    @Test func theInboxAsksOnlyForAllowedHosts() async throws {
        let harness = Harness()
        let account = Account(pluginID: "gitlab", name: nil, settings: ["host": "https://gitlab.acme.io"])
        try JSONEncoder().encode([account]).write(to: harness.folder.appending(path: "accounts.json"))
        let model = harness.model()
        let allowed = url("https://gitlab.acme.io/uploads/-/system/user/avatar/7/a.png")
        let other = url("https://evil.example.com/7.png")
        ImageServer.serve(.image(ImageServer.png), at: allowed)
        ImageServer.serve(.image(ImageServer.png), at: other)

        #expect(await model.image(at: allowed) != nil)
        #expect(model.cachedImage(allowed) != nil)
        #expect(await model.image(at: other) == nil)
        #expect(ImageServer.requests(to: other) == 0)

        model.preferences.allowRemoteImages = false
        #expect(model.cachedImage(allowed) == nil, "nothing shown while images are off")
        #expect(await model.image(at: allowed) == nil)
        #expect(ImageServer.requests(to: allowed) == 1)

        model.preferences.allowRemoteImages = true
        try model.eraseLocalData()
        #expect(harness.images.cached(allowed) == nil, "Erase forgets the pictures")
    }
}

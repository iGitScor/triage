import Foundation
import Network
import Testing

@testable import RemoraCore

/// Unchanged answers come back as 304 and are served from memory; identities are asked once an hour.
struct ConditionalRequestTests {
    /// A server whose answer carries `ETag: "v1"`, and that answers 304 when asked with it.
    final class Server: HTTPClient, @unchecked Sendable {
        private let lock = NSLock()
        private var _sent: [URLRequest] = []
        var sent: [URLRequest] { lock.withLock { _sent } }
        var etag: String? = #""v1""#

        func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
            lock.withLock { _sent.append(request) }
            let url = request.url!
            if let etag, request.value(forHTTPHeaderField: "If-None-Match") == etag {
                return (Data(), HTTPURLResponse(url: url, statusCode: 304, httpVersion: nil, headerFields: nil)!)
            }
            let headers = etag.map { ["ETag": $0] }
            return (
                Data("body of \(url.path)".utf8),
                HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: headers)!
            )
        }
    }

    let url = URL(string: "https://gitlab.acme.io/api/v4/merge_requests")!

    @Test func anUnchangedAnswerIsServedFromMemory() async throws {
        let server = Server()
        let client = ConditionalHTTPClient(server, store: ETagStore())
        let (first, _) = try await client.send(.get(url))
        let (second, response) = try await client.send(.get(url))

        #expect(server.sent[0].value(forHTTPHeaderField: "If-None-Match") == nil)
        #expect(server.sent[1].value(forHTTPHeaderField: "If-None-Match") == #""v1""#)
        #expect(response.statusCode == 200)
        #expect(second == first)
    }

    @Test func onlyGETsAreConditional() async throws {
        let server = Server()
        let client = ConditionalHTTPClient(server, store: ETagStore())
        let post = try URLRequest.post(url, json: ["query": "x"])
        _ = try await client.send(post)
        _ = try await client.send(post)
        #expect(server.sent.allSatisfy { $0.value(forHTTPHeaderField: "If-None-Match") == nil })
    }

    @Test func anAnswerWithoutETagIsNotKept() async throws {
        let server = Server()
        server.etag = nil
        let client = ConditionalHTTPClient(server, store: ETagStore())
        _ = try await client.send(.get(url))
        _ = try await client.send(.get(url))
        #expect(server.sent.allSatisfy { $0.value(forHTTPHeaderField: "If-None-Match") == nil })
    }

    @Test func theStoreKeepsTheLatestAnswersOnly() async {
        let store = ETagStore(limit: 2)
        let urls = (1...3).map { URL(string: "https://gitlab.acme.io/\($0)")! }
        for url in urls { await store.keep(.init(etag: "e", data: Data()), for: url) }
        #expect(await store.answer(for: urls[0]) == nil)
        #expect(await store.answer(for: urls[2]) != nil)
    }

    @Test func aValueIsKeptForItsLifetime() async throws {
        final class Clock: @unchecked Sendable { var now = Date(timeIntervalSince1970: 0) }
        let clock = Clock()
        let cache = MemoryAccountCache(now: { clock.now })
        final class Count: @unchecked Sendable { var loads = 0 }
        let count = Count()
        let load: @Sendable () async throws -> String = {
            count.loads += 1
            return "alice"
        }

        _ = try await cache.value("user", lifetime: 3600, load: load)
        clock.now = clock.now.addingTimeInterval(3599)
        _ = try await cache.value("user", lifetime: 3600, load: load)
        #expect(count.loads == 1)
        clock.now = clock.now.addingTimeInterval(1)
        _ = try await cache.value("user", lifetime: 3600, load: load)
        #expect(count.loads == 2)
    }

    @Test func aFailedLoadKeepsNothing() async throws {
        let cache = MemoryAccountCache()
        await #expect(throws: HTTPError.unauthorized) {
            try await cache.value("user", lifetime: 3600) { () async throws -> String in throw HTTPError.unauthorized }
        }
        #expect(try await cache.value("user", lifetime: 3600) { "alice" } == "alice")
    }

    @Test func aRenewedSecretIsTakenOnce() async {
        let cache = MemoryAccountCache()
        await cache.renew("refreshToken", "r2")
        #expect(await cache.takeRenewed() == ["refreshToken": "r2"])
        #expect(await cache.takeRenewed().isEmpty)
    }

    /// Through the real client and a socket, as Windows tests it: URLSession hands the 304 over (it isn't
    /// turned into a 200 from its own cache, nor followed), and the answer comes from memory.
    @Test func aRealServersNotModifiedIsServedFromMemory() async throws {
        let server = try await LocalServer.start()
        defer { server.stop() }
        let url = URL(string: "http://127.0.0.1:\(server.port)/api/v4/merge_requests")!
        let client = ConditionalHTTPClient(URLSessionHTTPClient(), store: ETagStore())
        let (first, _) = try await client.send(.get(url))
        let (second, response) = try await client.send(.get(url))
        #expect(String(decoding: first, as: UTF8.self) == "[1]")
        #expect(second == first && response.statusCode == 200)
        #expect(server.conditions == [nil, #""v1""#], "the second request asked with the ETag, and got a 304")
    }
}

/// An HTTP server on this Mac: `[1]` with `ETag: "v1"`, or 304 when asked with it. Records each If-None-Match.
final class LocalServer: @unchecked Sendable {
    private let listener: NWListener
    private let queue = DispatchQueue(label: "fr.igitscor.remora.tests.server")
    private let lock = NSLock()
    private var asked: [String?] = []
    let port: UInt16
    var conditions: [String?] { lock.withLock { asked } }

    private init(listener: NWListener, port: UInt16) {
        self.listener = listener
        self.port = port
    }

    static func start() async throws -> LocalServer {
        let parameters = NWParameters.tcp
        parameters.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
        let listener = try NWListener(using: parameters)
        let queue = DispatchQueue(label: "fr.igitscor.remora.tests.listener")
        let port: UInt16 = try await withCheckedThrowingContinuation { continuation in
            listener.stateUpdateHandler = { state in
                switch state {
                case .ready:
                    listener.stateUpdateHandler = nil
                    continuation.resume(returning: listener.port?.rawValue ?? 0)
                case .failed(let error):
                    listener.stateUpdateHandler = nil
                    continuation.resume(throwing: error)
                default: break
                }
            }
            listener.newConnectionHandler = { $0.cancel() }
            listener.start(queue: queue)
        }
        let server = LocalServer(listener: listener, port: port)
        listener.newConnectionHandler = { [server] in server.answer($0) }
        return server
    }

    func stop() { listener.cancel() }

    private func answer(_ connection: NWConnection) {
        connection.start(queue: queue)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 8192) { data, _, _, _ in
            let request = String(decoding: data ?? Data(), as: UTF8.self)
            let condition = request.components(separatedBy: "\r\n")
                .first { $0.lowercased().hasPrefix("if-none-match:") }
                .map { String($0.dropFirst("if-none-match:".count)).trimmingCharacters(in: .whitespaces) }
            self.lock.withLock { self.asked.append(condition) }
            let answer =
                condition == #""v1""#
                ? "HTTP/1.1 304 Not Modified\r\nETag: \"v1\"\r\nConnection: close\r\n\r\n"
                : "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nETag: \"v1\"\r\nContent-Length: 3\r\nConnection: close\r\n\r\n[1]"
            connection.send(content: Data(answer.utf8), completion: .contentProcessed { _ in connection.cancel() })
        }
    }
}

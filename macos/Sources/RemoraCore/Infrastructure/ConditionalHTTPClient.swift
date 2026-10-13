import Foundation

/// Conditional requests: a GET whose last answer carried an `ETag` is sent with `If-None-Match`, and a 304 is answered
/// with the copy kept from that last time, as a 200. Unchanged lists then cost the server no body. Only GETs are
/// conditional: GitLab's REST lists and Notion's reads gain, but GitHub and Linear are read with one GraphQL POST
/// each, which this doesn't cover (PERF-19). Sits behind the egress guard, so only allowed hosts are ever kept.
public struct ConditionalHTTPClient: HTTPClient {
    private let base: HTTPClient
    private let store: ETagStore

    public init(_ base: HTTPClient, store: ETagStore) {
        self.base = base
        self.store = store
    }

    public func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        guard (request.httpMethod ?? "GET") == "GET", let url = request.url else { return try await base.send(request) }
        var request = request
        let kept = await store.answer(for: url)
        if let kept { request.setValue(kept.etag, forHTTPHeaderField: "If-None-Match") }
        let (data, response) = try await base.send(request)
        if response.statusCode == 304, let kept,
            let unchanged = HTTPURLResponse(
                url: response.url ?? url, statusCode: 200, httpVersion: nil,
                headerFields: response.allHeaderFields as? [String: String])
        {
            return (kept.data, unchanged)
        }
        if (200..<300).contains(response.statusCode), let etag = response.value(forHTTPHeaderField: "ETag") {
            await store.keep(ETagStore.Answer(etag: etag, data: data), for: url)
        } else if kept != nil {
            await store.forget(url)
        }
        return (data, response)
    }
}

/// The last answer to each GET of one account that came with an `ETag`. In memory only: it holds message content,
/// which must not outlive "Erase local data". Bounded, the oldest answers going first.
public actor ETagStore {
    public struct Answer: Sendable {
        public var etag: String
        public var data: Data
    }

    private var answers: [URL: Answer] = [:]
    private var order: [URL] = []
    private let limit: Int

    public init(limit: Int = 256) {
        self.limit = limit
    }

    func answer(for url: URL) -> Answer? { answers[url] }

    func keep(_ answer: Answer, for url: URL) {
        if answers.updateValue(answer, forKey: url) == nil { order.append(url) }
        while order.count > limit { answers[order.removeFirst()] = nil }
    }

    func forget(_ url: URL) {
        answers[url] = nil
        order.removeAll { $0 == url }
    }
}

import Foundation
import RemoraCore

/// Answers requests with canned JSON, matched by the end of the URL path. A route with a `?` also needs that text in
/// the query (`"/merge_requests?scope=created_by_me"`); the longest matching route wins.
struct StubHTTP: HTTPClient {
    var routes: [String: String]

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let (path, query) = (request.url!.path, request.url!.query ?? "")
        func matches(_ route: String) -> Bool {
            let parts = route.split(separator: "?", maxSplits: 1).map(String.init)
            return path.hasSuffix(parts[0]) && (parts.count == 1 || query.contains(parts[1]))
        }
        guard let body = routes.filter({ matches($0.key) }).max(by: { $0.key.count < $1.key.count })?.value else {
            return (Data(), HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!)
        }
        return (
            Data(body.utf8), HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!
        )
    }
}

func config(_ values: [String: String]) -> PluginConfig {
    PluginConfig(accountID: UUID(), values: values)
}

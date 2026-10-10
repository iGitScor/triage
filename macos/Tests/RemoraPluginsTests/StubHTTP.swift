import Foundation
import RemoraCore

/// Answers requests with canned JSON, matched by the end of the URL path.
struct StubHTTP: HTTPClient {
    var routes: [String: String]

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let path = request.url!.path
        guard let body = routes.first(where: { path.hasSuffix($0.key) })?.value else {
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

import Foundation

public protocol HTTPClient: Sendable {
    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse)
}

public struct URLSessionHTTPClient: HTTPClient {
    private let session: URLSession

    public init(session: URLSession = .shared) {
        self.session = session
    }

    public func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw HTTPError.invalidResponse }
        return (data, http)
    }
}

public enum HTTPError: LocalizedError, Equatable {
    case invalidResponse
    case unauthorized
    case status(Int, String)
    case api(String)

    public var errorDescription: String? {
        switch self {
        case .invalidResponse: L("The server sent an invalid response.")
        case .unauthorized: L("The token was rejected. Check it and its scopes.")
        case .status(let code, let message): "HTTP \(code)\(message.isEmpty ? "" : ": \(message)")"
        case .api(let message): message
        }
    }
}

extension HTTPClient {
    public func decode<T: Decodable>(
        _ type: T.Type,
        from request: URLRequest,
        using decoder: JSONDecoder = .api()
    ) async throws -> T {
        let (data, response) = try await send(request)
        switch response.statusCode {
        case 200..<300:
            return try decoder.decode(T.self, from: data)
        case 401, 403:
            throw HTTPError.unauthorized
        default:
            let message = String(data: data.prefix(200), encoding: .utf8) ?? ""
            throw HTTPError.status(response.statusCode, message)
        }
    }
}

extension URLRequest {
    public static func get(_ url: URL, headers: [String: String] = [:]) -> URLRequest {
        var request = URLRequest(url: url, timeoutInterval: 30)
        headers.forEach { request.setValue($1, forHTTPHeaderField: $0) }
        request.setValue("Remora", forHTTPHeaderField: "User-Agent")
        return request
    }

    public static func post(_ url: URL, json body: some Encodable, headers: [String: String] = [:]) throws -> URLRequest {
        var request = get(url, headers: headers)
        request.httpMethod = "POST"
        request.httpBody = try JSONEncoder().encode(body)
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        return request
    }
}

extension URL {
    public func appending(path: String, query: [String: String]) -> URL {
        var components = URLComponents(url: appending(path: path), resolvingAgainstBaseURL: false)!
        components.queryItems = query.sorted { $0.key < $1.key }.map { URLQueryItem(name: $0.key, value: $0.value) }
        return components.url!
    }
}

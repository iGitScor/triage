import Foundation

public protocol HTTPClient: Sendable {
    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse)
}

public struct URLSessionHTTPClient: HTTPClient {
    private let session: URLSession

    public init(session: URLSession = Self.makeSession()) {
        self.session = session
    }

    /// Nothing on disk: responses hold message content and must not outlive "Erase local data". Redirects stay on
    /// the same host, over https: the egress guard only sees the first URL, and headers such as GitLab's
    /// `PRIVATE-TOKEN` would follow a redirect to another host.
    public static func makeSession() -> URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.urlCache = nil
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        return URLSession(configuration: configuration, delegate: SameHostRedirects(), delegateQueue: nil)
    }

    public func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw HTTPError.invalidResponse }
        return (data, http)
    }
}

/// Refuses redirects that leave the host or drop to http; the 3xx answer then fails in `decode`.
final class SameHostRedirects: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest
    ) async -> URLRequest? {
        RedirectPolicy.allows(from: response.url ?? task.currentRequest?.url, to: request.url) ? request : nil
    }
}

public enum RedirectPolicy {
    /// Same host (case-insensitive), and never from https to http.
    public static func allows(from: URL?, to: URL?) -> Bool {
        guard let fromHost = from?.host?.lowercased(), let toHost = to?.host?.lowercased(), fromHost == toHost else {
            return false
        }
        let fromScheme = from?.scheme?.lowercased()
        let toScheme = to?.scheme?.lowercased()
        return toScheme == "https" || (toScheme == "http" && fromScheme == "http")
    }
}

public enum HTTPError: LocalizedError, Equatable {
    /// What a failed status means for you, with its code for a bug report. Same text on Windows (`http.rs`).
    public static func describe(_ code: Int) -> String {
        let text =
            switch code {
            case 404: L("The server couldn’t find what Remora asked for: check the address and what the token can see.")
            case 500...: L("The server had a problem: Remora tries again at the next refresh.")
            default: L("The server refused the request.")
            }
        return "\(text) (HTTP \(code))"
    }

    case invalidResponse
    case unauthorized
    /// 429, or a 403 with no requests left. When to try again, if the server said.
    case rateLimited(Date?)
    /// Any other failure. Only the code: the body can echo the request and isn't meant for people.
    case status(Int)
    case api(String)

    public var errorDescription: String? {
        switch self {
        case .invalidResponse: L("The server sent an invalid response.")
        case .unauthorized: L("The token was rejected. Check it and its scopes.")
        case .rateLimited(let until?):
            L("Too many requests: Remora waits until %@.", until.formatted(date: .omitted, time: .shortened))
        case .rateLimited(nil): L("Too many requests: Remora waits a little before trying again.")
        case .status(let code): Self.describe(code)
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
        case 429:
            throw HTTPError.rateLimited(Self.resumeDate(response))
        case 403
        where ["x-ratelimit-remaining", "ratelimit-remaining"].contains {
            response.value(forHTTPHeaderField: $0) == "0"
        }:
            throw HTTPError.rateLimited(Self.resumeDate(response))
        case 401, 403:
            throw HTTPError.unauthorized
        case 300..<400:
            let target = response.value(forHTTPHeaderField: "Location").flatMap {
                URL(string: $0, relativeTo: response.url)?.host
            }
            throw EgressError.blockedRedirect(target ?? "?")
        default:
            throw HTTPError.status(response.statusCode)
        }
    }
}

extension HTTPClient {
    /// When a rate-limited server says to come back: `Retry-After` (seconds or a date), else the reset time
    /// GitHub (`x-ratelimit-reset`) and GitLab (`RateLimit-Reset`) give in seconds since 1970.
    static func resumeDate(_ response: HTTPURLResponse, now: Date = .now) -> Date? {
        if let retry = response.value(forHTTPHeaderField: "Retry-After") {
            if let seconds = TimeInterval(retry.trimmingCharacters(in: .whitespaces)) {
                return now.addingTimeInterval(seconds)
            }
            let formatter = DateFormatter()
            formatter.locale = Locale(identifier: "en_US_POSIX")
            formatter.dateFormat = "EEE, dd MMM yyyy HH:mm:ss zzz"
            if let date = formatter.date(from: retry) { return date }
        }
        for header in ["x-ratelimit-reset", "ratelimit-reset"] {
            if let value = response.value(forHTTPHeaderField: header), let epoch = TimeInterval(value) {
                return Date(timeIntervalSince1970: epoch)
            }
        }
        return nil
    }
}

extension URLRequest {
    /// `timeout` is how long the server may stay silent. A non-streamed answer is silent until it is complete,
    /// so a call that generates text (Claude) needs far more than the 30 s that suit the tools' APIs.
    public static func get(_ url: URL, headers: [String: String] = [:], timeout: TimeInterval = 30) -> URLRequest {
        var request = URLRequest(url: url, timeoutInterval: timeout)
        for (field, value) in headers { request.setValue(value, forHTTPHeaderField: field) }
        request.setValue("Remora", forHTTPHeaderField: "User-Agent")
        return request
    }

    public static func post(
        _ url: URL, json body: some Encodable, headers: [String: String] = [:], timeout: TimeInterval = 30
    ) throws -> URLRequest {
        var request = get(url, headers: headers, timeout: timeout)
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

import Foundation

/// Lets a plugin reach only the hosts it declared. Every plugin request goes through one of these.
public struct GuardedHTTPClient: HTTPClient {
    private let base: HTTPClient
    private let hosts: [String]

    public init(_ base: HTTPClient, allowing hosts: [String]) {
        self.base = base
        self.hosts = hosts.map { $0.lowercased() }
    }

    public func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let host = request.url?.host?.lowercased() ?? ""
        guard Self.matches(host, hosts) else { throw EgressError.blockedHost(host.isEmpty ? "?" : host) }
        return try await base.send(request)
    }

    public static func matches(_ host: String, _ hosts: [String]) -> Bool {
        !host.isEmpty && hosts.contains { host == $0 || host.hasSuffix("." + $0) }
    }
}

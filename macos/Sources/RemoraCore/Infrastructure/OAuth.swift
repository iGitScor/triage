import CryptoKit
import Foundation
import Network

/// Signing in through the browser (`SignIn`): OAuth 2's authorization code with PKCE (RFC 7636), for an app that
/// keeps no secret. The browser comes back to a port on this Mac (RFC 8252), open only while it waits. Same steps
/// on Windows (`oauth.rs`).
public enum OAuth {
    /// The secret only this sign-in knows, and what the authorization server is shown of it.
    public struct PKCE: Sendable {
        public let verifier: String
        public let challenge: String

        public init(verifier: String = OAuth.random(32)) {
            self.verifier = verifier
            challenge = OAuth.base64url(Data(SHA256.hash(data: Data(verifier.utf8))))
        }
    }

    /// `bytes` random bytes, URL-safe.
    public static func random(_ bytes: Int) -> String {
        var buffer = [UInt8](repeating: 0, count: bytes)
        precondition(SecRandomCopyBytes(kSecRandomDefault, bytes, &buffer) == errSecSuccess)
        return base64url(Data(buffer))
    }

    /// Base64 with `-` and `_`, unpadded.
    public static func base64url(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    /// The URL with each `{key}` replaced by that setting. Letters, digits, dots and dashes only, so a setting
    /// can't change where the URL goes.
    public static func fill(_ template: String, _ values: [String: String]) throws -> URL {
        var url = template
        while let start = url.firstIndex(of: "{") {
            guard let end = url[start...].firstIndex(of: "}") else { throw PluginError.invalidField(template) }
            let key = String(url[url.index(after: start)..<end])
            let value = try setting(values, key)
            let allowed = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-")
            guard value.unicodeScalars.allSatisfy(allowed.contains) else { throw PluginError.invalidField(key) }
            url.replaceSubrange(start...end, with: value)
        }
        guard let filled = URL(string: url) else { throw PluginError.invalidField(template) }
        return filled
    }

    static func setting(_ values: [String: String], _ key: String) throws -> String {
        let value = (values[key] ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty else { throw PluginError.missingField(key) }
        return value
    }

    /// Where the browser goes to sign in.
    public static func authorizeURL(
        _ signIn: SignIn, values: [String: String], redirect: String, pkce: PKCE, state: String
    ) throws -> URL {
        var components = URLComponents(url: try fill(signIn.authorizeURL, values), resolvingAgainstBaseURL: false)!
        components.queryItems = [
            ("client_id", try setting(values, signIn.clientIDKey)),
            ("response_type", "code"),
            ("redirect_uri", redirect),
            ("response_mode", "query"),
            ("scope", signIn.scopes.joined(separator: " ")),
            ("state", state),
            ("code_challenge", pkce.challenge),
            ("code_challenge_method", "S256"),
            ("prompt", "select_account"),
        ].map { URLQueryItem(name: $0.0, value: $0.1) }
        // `+` is a space in a query to most servers: encoded, it stays a plus.
        components.percentEncodedQuery = components.percentEncodedQuery?.replacingOccurrences(of: "+", with: "%2B")
        return components.url!
    }

    /// What the token endpoint gives.
    public struct Tokens: Decodable, Equatable, Sendable {
        public var accessToken: String
        /// Absent when the server keeps the previous one.
        public var refreshToken: String?
        /// Seconds.
        public var expiresIn: Int?

        enum CodingKeys: String, CodingKey {
            case accessToken = "access_token"
            case refreshToken = "refresh_token"
            case expiresIn = "expires_in"
        }
    }

    /// Trades the code the browser brought back for tokens.
    public static func redeem(
        _ signIn: SignIn, values: [String: String], code: String, redirect: String, verifier: String,
        http: HTTPClient
    ) async throws -> Tokens {
        try await tokens(
            signIn, values: values,
            fields: [
                ("grant_type", "authorization_code"), ("code", code), ("redirect_uri", redirect),
                ("code_verifier", verifier),
            ], http: http)
    }

    /// A new access token from the refresh token, and maybe a new refresh token.
    public static func refresh(
        _ signIn: SignIn, values: [String: String], refreshToken: String, http: HTTPClient
    ) async throws -> Tokens {
        try await tokens(
            signIn, values: values, fields: [("grant_type", "refresh_token"), ("refresh_token", refreshToken)],
            http: http)
    }

    private static func tokens(
        _ signIn: SignIn, values: [String: String], fields: [(String, String)], http: HTTPClient
    ) async throws -> Tokens {
        let form =
            [("client_id", try setting(values, signIn.clientIDKey))] + fields + [
                ("scope", signIn.scopes.joined(separator: " "))
            ]
        let (data, response) = try await http.send(.post(try fill(signIn.tokenURL, values), form: form))
        if (200..<300).contains(response.statusCode) {
            do { return try JSONDecoder().decode(Tokens.self, from: data) } catch {
                throw HTTPError.api(L("Unexpected answer from the server: %@", error.localizedDescription))
            }
        }
        struct Refusal: Decodable { var error: String }
        // Only the error's code: its description can echo the request.
        switch (try? JSONDecoder().decode(Refusal.self, from: data))?.error {
        // The sign-in expired, or was revoked: only signing in again helps.
        case "invalid_grant": throw HTTPError.unauthorized
        case let error?: throw PluginError.signIn(error)
        case nil where response.statusCode == 401: throw HTTPError.unauthorized
        case nil: throw HTTPError.status(response.statusCode)
        }
    }

    /// An access token for the account: the one kept until a few minutes before it expires, else a new one from the
    /// refresh token. A refresh token the server rotated is used from then on, and handed to the app to save
    /// (`AccountCache.renew`). Call it once per fetch, before requests run in parallel.
    public static func accessToken(
        _ signIn: SignIn, config: PluginConfig, http: HTTPClient, now: @Sendable () -> Date = { .now }
    ) async throws -> String {
        let cache = config.cache
        let kept: KeptToken? = await cache.kept("oauth.access")
        if let kept, now() < kept.until { return kept.token }
        let renewed: String? = await cache.kept("oauth.refresh")
        let current = try renewed ?? config.required("refreshToken")
        let tokens = try await refresh(signIn, values: config.values, refreshToken: current, http: http)
        if let rotated = tokens.refreshToken, rotated != current {
            await cache.keep("oauth.refresh", rotated)
            await cache.renew("refreshToken", rotated)
        }
        let lifetime = max(60, (tokens.expiresIn ?? 3600) - 300)
        await cache.keep(
            "oauth.access",
            KeptToken(token: tokens.accessToken, until: now().addingTimeInterval(TimeInterval(lifetime))))
        return tokens.accessToken
    }

    struct KeptToken: Sendable {
        var token: String
        var until: Date
    }

    /// What a request to the loopback port says.
    public enum Callback: Equatable {
        case code(String)
        case refused(String)
        /// Not the sign-in coming back (a favicon, another tab).
        case other
    }

    /// Reads `GET /?code=…&state=… HTTP/1.1`. A different `state` isn't this sign-in's answer.
    public static func callback(_ requestLine: String, state: String) -> Callback {
        let parts = requestLine.split(separator: " ")
        guard parts.count >= 2, parts[0] == "GET", let url = URLComponents(string: "http://localhost\(parts[1])")
        else { return .other }
        var query: [String: String] = [:]
        for item in url.queryItems ?? [] { query[item.name] = item.value ?? "" }
        guard url.path == "/", query["state"] == state else { return .other }
        if let error = query["error"] { return .refused(error) }
        if let code = query["code"], !code.isEmpty { return .code(code) }
        return .refused("no code")
    }
}

extension URLRequest {
    /// A form, as OAuth's token endpoints take it.
    public static func post(_ url: URL, form fields: [(String, String)]) -> URLRequest {
        var request = get(url)
        request.httpMethod = "POST"
        var allowed = CharacterSet.alphanumerics
        allowed.insert(charactersIn: "-._~")
        request.httpBody = Data(
            fields.map {
                "\($0.0)=\($0.1.addingPercentEncoding(withAllowedCharacters: allowed) ?? "")"
            }.joined(separator: "&").utf8)
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        return request
    }
}

/// A port on this Mac that waits for the browser to come back.
public final class Loopback: @unchecked Sendable {
    /// `http://localhost:<port>`: the organization registers `http://localhost`, any port.
    public let redirect: String
    private let listener: NWListener
    private let queue: DispatchQueue
    // On `queue` only.
    private var outcome: Result<String, Error>?
    private var waiting: CheckedContinuation<String, Error>?
    /// This sign-in's `state`: another one's answer isn't taken.
    private let expected: String

    private init(listener: NWListener, queue: DispatchQueue, port: UInt16, state: String) {
        self.listener = listener
        self.queue = queue
        expected = state
        redirect = "http://localhost:\(port)"
    }

    public static func open(state: String) async throws -> Loopback {
        let parameters = NWParameters.tcp
        parameters.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
        let listener = try NWListener(using: parameters)
        let queue = DispatchQueue(label: "fr.igitscor.remora.loopback")
        let port: UInt16 = try await withCheckedThrowingContinuation { continuation in
            // Called on `queue`, one state at a time: cleared once it has answered.
            listener.stateUpdateHandler = { state in
                switch state {
                case .ready:
                    listener.stateUpdateHandler = nil
                    continuation.resume(returning: listener.port?.rawValue ?? 0)
                case .failed(let error):
                    listener.stateUpdateHandler = nil
                    continuation.resume(throwing: PluginError.signIn(error.localizedDescription))
                default: break
                }
            }
            listener.newConnectionHandler = { $0.cancel() }
            listener.start(queue: queue)
        }
        let loopback = Loopback(listener: listener, queue: queue, port: port, state: state)
        listener.newConnectionHandler = { [weak loopback] in loopback?.answer($0) }
        return loopback
    }

    /// The code the browser brings back. Waits until then, or until the task is cancelled.
    public func code() async throws -> String {
        defer { listener.cancel() }
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                queue.async {
                    if Task.isCancelled { self.outcome = .failure(CancellationError()) }
                    if let outcome = self.outcome {
                        continuation.resume(with: outcome)
                    } else {
                        self.waiting = continuation
                    }
                }
            }
        } onCancel: {
            queue.async { self.finish(.failure(CancellationError())) }
        }
    }

    private func finish(_ result: Result<String, Error>) {
        guard outcome == nil else { return }
        outcome = result
        waiting?.resume(with: result)
        waiting = nil
    }

    private func answer(_ connection: NWConnection) {
        connection.start(queue: queue)
        read(connection, Data())
    }

    private func read(_ connection: NWConnection, _ received: Data) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 4096) { data, _, done, error in
            let received = received + (data ?? Data())
            if !received.contains(Data("\r\n\r\n".utf8)), received.count < 8192, !done, error == nil {
                return self.read(connection, received)
            }
            let line = String(decoding: received, as: UTF8.self).components(separatedBy: "\r\n").first ?? ""
            let callback = OAuth.callback(line, state: self.expected)
            let (status, page) =
                switch callback {
                case .other: ("404 Not Found", "")
                case .code: ("200 OK", Self.signedIn)
                case .refused: ("200 OK", Self.notSignedIn)
                }
            let answer =
                "HTTP/1.1 \(status)\r\nContent-Type: text/html; charset=utf-8\r\n"
                + "Content-Length: \(page.utf8.count)\r\nConnection: close\r\n\r\n\(page)"
            connection.send(content: Data(answer.utf8), completion: .contentProcessed { _ in connection.cancel() })
            switch callback {
            case .code(let code): self.finish(.success(code))
            case .refused(let error): self.finish(.failure(PluginError.signIn(error)))
            case .other: break
            }
        }
    }

    private static let signedIn = """
        <!doctype html><meta charset=utf-8><title>Remora</title>\
        <body style="font:16px system-ui;margin:3em;text-align:center"><h1>✓</h1>\
        <p>You’re signed in: you can close this tab and go back to Remora.</p>\
        <p lang=fr>Vous êtes connecté : vous pouvez fermer cet onglet et revenir à Remora.</p>
        """

    private static let notSignedIn = """
        <!doctype html><meta charset=utf-8><title>Remora</title>\
        <body style="font:16px system-ui;margin:3em;text-align:center">\
        <p>The sign-in didn’t finish: Remora says why.</p>\
        <p lang=fr>La connexion n’a pas abouti : Remora vous dit pourquoi.</p>
        """
}

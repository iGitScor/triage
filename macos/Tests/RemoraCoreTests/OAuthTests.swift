import Foundation
import Testing

@testable import RemoraCore

struct OAuthTests {
    let microsoft = SignIn(
        authorizeURL: "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize",
        tokenURL: "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token",
        clientIDKey: "clientId",
        scopes: ["offline_access", "Mail.Read"]
    )

    func values(_ tenant: String) -> [String: String] { ["tenant": tenant, "clientId": "app-1"] }

    @Test func pkceFollowsTheRFC() {
        // RFC 7636, appendix B.
        #expect(
            OAuth.PKCE(verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk").challenge
                == "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM")
        #expect(OAuth.base64url(Data([0xfb, 0xff])) == "-_8")
        let (one, two) = (OAuth.PKCE(), OAuth.PKCE())
        #expect(one.verifier.count == 43)
        #expect(one.verifier != two.verifier)
    }

    @Test func theAuthorizeURLCarriesTheChallengeAndTheTenant() throws {
        let pkce = OAuth.PKCE()
        let url = try OAuth.authorizeURL(
            microsoft, values: values("acme.com"), redirect: "http://localhost:5050", pkce: pkce, state: "s1")
        #expect(url.path == "/acme.com/oauth2/v2.0/authorize")
        let query = Dictionary(
            uniqueKeysWithValues: (URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []).map {
                ($0.name, $0.value ?? "")
            })
        #expect(query["client_id"] == "app-1")
        #expect(query["redirect_uri"] == "http://localhost:5050")
        #expect(query["scope"] == "offline_access Mail.Read")
        #expect(query["code_challenge"] == pkce.challenge)
        #expect(query["code_challenge_method"] == "S256")
        #expect(query["state"] == "s1")
    }

    @Test func aSettingCantChangeWhereTheURLGoes() {
        for tenant in ["evil.com/x?", "a b", "x#"] {
            #expect(throws: PluginError.invalidField("tenant")) {
                try OAuth.authorizeURL(
                    microsoft, values: values(tenant), redirect: "http://localhost:1", pkce: .init(), state: "s")
            }
        }
        #expect(throws: PluginError.missingField("tenant")) {
            try OAuth.authorizeURL(
                microsoft, values: values(" "), redirect: "http://localhost:1", pkce: .init(), state: "s")
        }
    }

    @Test func readsTheBrowserComingBack() {
        #expect(OAuth.callback("GET /?code=c%2B1&state=s1 HTTP/1.1", state: "s1") == .code("c+1"))
        #expect(OAuth.callback("GET /?error=access_denied&state=s1 HTTP/1.1", state: "s1") == .refused("access_denied"))
        #expect(OAuth.callback("GET /?code=c&state=other HTTP/1.1", state: "s1") == .other)
        #expect(OAuth.callback("GET /favicon.ico HTTP/1.1", state: "s1") == .other)
        #expect(OAuth.callback("POST /?code=c&state=s1 HTTP/1.1", state: "s1") == .other)
    }

    @Test func theLoopbackWaitsForThisSignIn() async throws {
        let loopback = try await Loopback.open(state: "s1")
        let base = loopback.redirect.replacingOccurrences(of: "localhost", with: "127.0.0.1")
        let session = URLSession(configuration: .ephemeral)
        let (_, favicon) = try await session.data(from: URL(string: base + "/favicon.ico")!)
        #expect((favicon as? HTTPURLResponse)?.statusCode == 404)
        let (page, _) = try await session.data(from: URL(string: base + "/?code=c1&state=s1")!)
        #expect(String(decoding: page, as: UTF8.self).contains("You’re signed in"))
        #expect(try await loopback.code() == "c1")
    }

    @Test func aCancelledWaitStops() async throws {
        let loopback = try await Loopback.open(state: "s1")
        let waiting = Task { try await loopback.code() }
        waiting.cancel()
        await #expect(throws: CancellationError.self) { try await waiting.value }
    }

    /// Answers the token endpoint with `status` and `body`, keeping the request it was sent.
    final class TokenServer: HTTPClient, @unchecked Sendable {
        let status: Int
        let body: String
        var sent: URLRequest?

        init(_ status: Int, _ body: String) {
            self.status = status
            self.body = body
        }

        func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
            sent = request
            return (
                Data(body.utf8),
                HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: nil)!
            )
        }
    }

    @Test func redeemsTheCodeWithTheVerifier() async throws {
        let http = TokenServer(200, #"{"access_token": "a1", "refresh_token": "r1", "expires_in": 3600}"#)
        let tokens = try await OAuth.redeem(
            microsoft, values: values("acme.com"), code: "c 1", redirect: "http://localhost:5050", verifier: "v1",
            http: http)
        #expect(tokens == OAuth.Tokens(accessToken: "a1", refreshToken: "r1", expiresIn: 3600))
        let sent = try #require(http.sent)
        #expect(sent.url?.absoluteString == "https://login.microsoftonline.com/acme.com/oauth2/v2.0/token")
        #expect(sent.value(forHTTPHeaderField: "Content-Type") == "application/x-www-form-urlencoded")
        let form = Dictionary(
            uniqueKeysWithValues: (URLComponents(
                string: "http://x/?" + String(decoding: sent.httpBody ?? Data(), as: UTF8.self))?.queryItems ?? []).map
            { ($0.name, $0.value ?? "") })
        #expect(form["grant_type"] == "authorization_code")
        #expect(form["code"] == "c 1")
        #expect(form["code_verifier"] == "v1")
        #expect(form["redirect_uri"] == "http://localhost:5050")
        #expect(form["scope"] == "offline_access Mail.Read")
    }

    @Test func anExpiredSignInAsksToSignInAgain() async {
        let expired = TokenServer(400, #"{"error": "invalid_grant", "error_description": "AADSTS70008: expired"}"#)
        await #expect(throws: HTTPError.unauthorized) {
            try await OAuth.refresh(microsoft, values: values("acme.com"), refreshToken: "r0", http: expired)
        }
        let unknownApp = TokenServer(400, #"{"error": "unauthorized_client"}"#)
        await #expect(throws: PluginError.signIn("unauthorized_client")) {
            try await OAuth.refresh(microsoft, values: values("acme.com"), refreshToken: "r0", http: unknownApp)
        }
        await #expect(throws: HTTPError.status(404)) {
            try await OAuth.refresh(
                microsoft, values: values("acme.com"), refreshToken: "r0", http: TokenServer(404, ""))
        }
    }
}

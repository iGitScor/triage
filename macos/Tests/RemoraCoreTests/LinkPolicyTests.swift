import Foundation
import Testing
@testable import RemoraCore

struct LinkPolicyTests {
    @Test func opensOnlyWebPagesAndToolApps() throws {
        #expect(LinkPolicy.isWebLink(try #require(URL(string: "https://github.com/acme/app/pull/1"))))
        for raw in ["file:///etc/passwd", "smb://evil/share", "javascript:alert(1)", "x-apple.systempreferences:", "ftp://h/x", "https:///nohost"] {
            #expect(!LinkPolicy.isWebLink(try #require(URL(string: raw))), "\(raw)")
        }
        #expect(LinkPolicy.isAppLink(try #require(URL(string: "slack://channel?team=T1&id=C1"))))
        #expect(LinkPolicy.isAppLink(try #require(URL(string: "linear:/acme/issue/ENG-1"))))
        #expect(!LinkPolicy.isAppLink(try #require(URL(string: "file:///Applications/Calculator.app"))))
    }

    @Test func httpOnlyOnTheAccountsOwnHost() throws {
        let gitlab = Account(pluginID: "gitlab", settings: ["host": "http://gitlab.lan"])
        let hosts = LinkPolicy.httpHosts(of: gitlab)
        #expect(LinkPolicy.isWebLink(try #require(URL(string: "http://gitlab.lan/a/b/-/merge_requests/3")), httpHosts: hosts))
        #expect(!LinkPolicy.isWebLink(try #require(URL(string: "http://elsewhere.io/x")), httpHosts: hosts))
        #expect(LinkPolicy.httpHosts(of: Account(pluginID: "gitlab", settings: ["host": "gitlab.com"])).isEmpty)
    }

    @Test func redirectsStayOnTheHostOverHTTPS() {
        let url = { (s: String) in URL(string: s) }
        #expect(RedirectPolicy.allows(from: url("https://gitlab.acme.io/api/v4/x"), to: url("https://GitLab.acme.io/api/v4/y")))
        #expect(RedirectPolicy.allows(from: url("http://gitlab.lan/a"), to: url("https://gitlab.lan/a")))
        #expect(!RedirectPolicy.allows(from: url("https://gitlab.acme.io/a"), to: url("https://collector.evil/a")))
        #expect(!RedirectPolicy.allows(from: url("https://gitlab.acme.io/a"), to: url("https://evil.gitlab.acme.io/a")))
        #expect(!RedirectPolicy.allows(from: url("https://gitlab.acme.io/a"), to: url("http://gitlab.acme.io/a")))
        #expect(!RedirectPolicy.allows(from: nil, to: url("https://gitlab.acme.io/a")))
    }

    @Test func theSessionKeepsNothingOnDisk() {
        let configuration = URLSessionHTTPClient.makeSession().configuration
        #expect(configuration.urlCache == nil)
        #expect(configuration.requestCachePolicy == .reloadIgnoringLocalCacheData)
    }

    @Test func aRefusedRedirectIsReportedAsBlocked() async throws {
        struct Redirect: HTTPClient {
            func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
                let response = HTTPURLResponse(url: request.url!, statusCode: 302, httpVersion: nil, headerFields: ["Location": "https://collector.evil/x"])!
                return (Data(), response)
            }
        }
        let request = URLRequest.get(URL(string: "https://gitlab.acme.io/api/v4/user")!)
        await #expect(throws: EgressError.blockedRedirect("collector.evil")) {
            _ = try await Redirect().decode([String: String].self, from: request)
        }
    }
}

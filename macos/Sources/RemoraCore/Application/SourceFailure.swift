import Foundation

/// Why a source failed, so the inbox can say it calmly and offer the fix that applies. Being offline isn't the
/// tool's fault, a rejected token needs reconnecting, and a rate limit only means waiting.
public enum FailureKind: Equatable, Sendable {
    /// No network at all: every source fails the same way.
    case offline
    /// This host can't be reached (a VPN, DNS, the server down) while the network works.
    case unreachable
    /// The token was rejected: reconnect the account.
    case auth
    case rateLimited
    case other

    public init(_ error: Error) {
        switch error {
        case HTTPError.unauthorized:
            self = .auth
        case HTTPError.rateLimited:
            self = .rateLimited
        case let error as URLError:
            switch error.code {
            case .notConnectedToInternet, .networkConnectionLost, .dataNotAllowed, .internationalRoamingOff, .callIsActive:
                self = .offline
            case .cannotFindHost, .cannotConnectToHost, .dnsLookupFailed, .timedOut, .secureConnectionFailed:
                self = .unreachable
            default:
                self = .other
            }
        default:
            self = .other
        }
    }
}

/// A source's last failure: its kind, and what to tell the user.
public struct SourceFailure: Equatable, Sendable {
    public let kind: FailureKind
    public let message: String

    public init(kind: FailureKind, message: String) {
        self.kind = kind
        self.message = message
    }

    public init(_ error: Error) {
        let kind = FailureKind(error)
        self.kind = kind
        // The system's wording for a network failure is long and technical; these say what it means here.
        message = switch kind {
        case .offline: L("Offline: Remora tries again when the connection is back.")
        case .unreachable: L("Can’t reach the server. Check the address, or your VPN if it needs one.")
        default: error.localizedDescription
        }
    }
}

/// What the inbox footer says about the sources, most important first.
public enum SourcesHealth: Equatable, Sendable {
    case fine
    /// No network, or every source failed for lack of one.
    case offline
    /// Tokens to reconnect, by account.
    case reconnect([UUID])
    /// Sources failing for another reason.
    case failing(Int)

    public init(failures: [UUID: SourceFailure], offline: Bool) {
        if offline || (!failures.isEmpty && failures.values.allSatisfy { $0.kind == .offline }) {
            self = .offline
            return
        }
        let auth = failures.filter { $0.value.kind == .auth }.map(\.key).sorted { $0.uuidString < $1.uuidString }
        if !auth.isEmpty {
            self = .reconnect(auth)
            return
        }
        // A rate limit is waited out, and the account says until when: not a failure.
        let failing = failures.values.filter { $0.kind != .rateLimited && $0.kind != .offline }.count
        self = failing > 0 ? .failing(failing) : .fine
    }
}

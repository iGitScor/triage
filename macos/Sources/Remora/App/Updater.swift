import Foundation
import Observation
import RemoraCore

/// What updating reaches outside the app: GitHub, the disk and the app bundle in the app; stand-ins in tests.
@MainActor
protocol UpdateSystem {
    /// The running version, and the release key from Info.plist (nil in builds that can't update, such as dev builds).
    var currentVersion: AppVersion? { get }
    var publicKey: String? { get }
    func manifest() async throws -> UpdateManifest
    /// Downloads to a local file.
    func download(_ url: URL) async throws -> URL
    /// Puts the verified app from the disk image in place of the running one.
    func install(_ dmg: URL, version: AppVersion) async throws
    func relaunch()
}

/// Remora updates itself only when you, or your organization, turn it on. Until then it never calls GitHub,
/// except when you press Check now. A download is installed only if the release workflow signed it (Ed25519), it is
/// newer, and the new app is signed by the same certificate as this one.
@MainActor
@Observable
final class Updater {
    enum State: Equatable {
        case idle
        case checking
        case upToDate
        case available(UpdateOffer)
        case installing(UpdateOffer)
        case failed(String)
    }

    static let interval: TimeInterval = 24 * 60 * 60

    private(set) var state = State.idle
    private(set) var lastCheck: Date?
    @ObservationIgnored private let system: UpdateSystem
    @ObservationIgnored private var schedule: Task<Void, Never>?

    init(system: UpdateSystem) {
        self.system = system
    }

    /// False in builds that carry no release key: they can't check a download, so they don't offer updates.
    var isSupported: Bool { system.publicKey != nil && system.currentVersion != nil }

    var offer: UpdateOffer? {
        switch state {
        case .available(let offer), .installing(let offer): offer
        default: nil
        }
    }

    /// Checks now, then once a day, while `enabled`; stops when it isn't.
    func schedule(enabled: Bool) {
        schedule?.cancel()
        schedule = nil
        guard enabled, isSupported else { return }
        schedule = Task { [weak self] in
            while !Task.isCancelled {
                await self?.check()
                try? await Task.sleep(for: .seconds(Self.interval))
            }
        }
    }

    func check() async {
        guard isSupported, let current = system.currentVersion else { return }
        if case .checking = state { return }
        if case .installing = state { return }
        state = .checking
        do {
            let manifest = try await system.manifest()
            lastCheck = .now
            state = UpdateOffer.from(manifest, current: current).map(State.available) ?? .upToDate
        } catch {
            state = .failed(L("Couldn’t check for updates: %@", error.localizedDescription))
        }
    }

    /// Downloads, verifies and installs the offer, then relaunches. Nothing changes on disk unless every check passes.
    func install() async {
        guard case .available(let offer) = state, let publicKey = system.publicKey else { return }
        state = .installing(offer)
        do {
            let dmg = try await system.download(offer.url)
            defer { try? FileManager.default.removeItem(at: dmg) }
            let data = try Data(contentsOf: dmg, options: .mappedIfSafe)
            guard UpdateSignature.isValid(data, signature: offer.signature, publicKey: publicKey) else {
                throw UpdateError.badSignature
            }
            try await system.install(dmg, version: offer.version)
            system.relaunch()
        } catch {
            state = .failed(error.localizedDescription)
        }
    }

    func forget() {
        schedule(enabled: false)
        state = .idle
        lastCheck = nil
    }
}

enum UpdateError: LocalizedError, Equatable {
    case badSignature
    case wrongApp
    case wrongVersion
    case notSameDeveloper
    case cannotReplace(String)

    var errorDescription: String? {
        switch self {
        case .badSignature:
            L("The download isn’t signed by Remora’s release key, so it wasn’t installed.")
        case .wrongApp, .wrongVersion:
            L("The download doesn’t contain the expected version of Remora, so it wasn’t installed.")
        case .notSameDeveloper:
            L(
                "This copy of Remora can’t update itself (it isn’t signed like the releases). Download the new version from the releases page."
            )
        case .cannotReplace(let folder):
            L(
                "Remora can’t replace itself in %@. Move it to Applications, or download the new version from the releases page.",
                folder)
        }
    }
}

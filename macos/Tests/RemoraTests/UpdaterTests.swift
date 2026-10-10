import Foundation
import RemoraCore
import Testing

@testable import Remora

/// Updates are opt-in, the organization can lock them, and only a signed, newer release is installed.
@MainActor
struct UpdaterTests {
    @Test func nothingCallsHomeUntilTurnedOn() async throws {
        let harness = Harness()
        let model = harness.model()
        #expect(!model.preferences.checkForUpdates, "off by default")
        model.start()
        try await Task.sleep(for: .milliseconds(50))
        #expect(harness.updates.manifestRequests == 0)

        model.preferences.checkForUpdates = true
        try await Task.sleep(for: .milliseconds(50))
        #expect(harness.updates.manifestRequests == 1, "checks as soon as it's on")
        #expect(model.updater.offer?.version.description == "0.4.0")
        #expect(harness.model().preferences.checkForUpdates, "saved")
    }

    @Test func checkNowWorksWithAutomaticChecksOff() async {
        let harness = Harness()
        let model = harness.model()
        await model.checkForUpdates()
        #expect(harness.updates.manifestRequests == 1)
        harness.updates.published = "0.3.2"
        await model.checkForUpdates()
        #expect(model.updater.state == .upToDate)
    }

    @Test func theOrganizationCanTurnUpdatingOff() async throws {
        var harness = Harness()
        harness.managed.values[ManagedPolicy.automaticUpdatesKey] = false
        let model = harness.model()
        model.preferences.checkForUpdates = true
        await model.checkForUpdates()
        try await Task.sleep(for: .milliseconds(50))
        #expect(harness.updates.manifestRequests == 0, "not even Check now")
        #expect(!model.effectivePreferences.checkForUpdates)
    }

    @Test func theOrganizationCanTurnAutomaticChecksOn() async throws {
        var harness = Harness()
        harness.managed.values[ManagedPolicy.automaticUpdatesKey] = true
        let model = harness.model()
        #expect(model.effectivePreferences.checkForUpdates)
    }

    @Test func aSignedNewerReleaseIsInstalledThenRelaunched() async {
        let harness = Harness()
        let model = harness.model()
        await model.checkForUpdates()
        await model.updater.install()
        #expect(harness.updates.installed?.description == "0.4.0")
        #expect(harness.updates.relaunched)
    }

    @Test func aDownloadThatDoesntMatchItsSignatureIsNeverInstalled() async {
        let harness = Harness()
        harness.updates.signedFile = Data("what was signed".utf8)
        let model = harness.model()
        await model.checkForUpdates()
        await model.updater.install()
        #expect(harness.updates.installed == nil && !harness.updates.relaunched)
        #expect(model.updater.state == .failed(UpdateError.badSignature.localizedDescription))
    }

    @Test func aBuildWithoutTheReleaseKeyDoesntOfferUpdates() async {
        let harness = Harness()
        harness.updates.publicKey = nil
        let model = harness.model()
        await model.checkForUpdates()
        #expect(!model.updater.isSupported)
        #expect(harness.updates.manifestRequests == 0)
    }
}

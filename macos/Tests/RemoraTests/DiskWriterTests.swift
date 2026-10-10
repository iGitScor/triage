import Foundation
import RemoraCore
import Testing

@testable import Remora

/// PERF-06: changes are saved off the main thread, each file once per burst, and nothing pending is lost.
@MainActor
struct DiskWriterTests {
    /// The files written from now on, in order: `flushWrites` reports its own writes before returning.
    private func recordWrites(_ model: InboxModel) -> () -> [String] {
        var written: [String] = []
        let original = model.writer.onResult
        model.writer.onResult = { file, failure in
            written.append(file)
            original(file, failure)
        }
        return {
            model.flushWrites()
            return written
        }
    }

    @Test func markAllAsDoneSavesEachFileOnce() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        model.flushWrites()
        let written = recordWrites(model)
        let items = model.allItems
        model.sweep(items)
        #expect(written().filter { $0 == "states.json" }.count == 1)
        let relaunched = harness.model()
        #expect(items.allSatisfy { relaunched.state(of: $0).done != nil }, "every item saved as done")
    }

    @Test func theLatestValueWins() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        let item = try #require(model.allItems.first)
        model.flushWrites()
        let written = recordWrites(model)
        model.togglePin(item)
        model.togglePin(item)
        model.togglePin(item)
        #expect(written() == ["states.json"])
        #expect(harness.model().state(of: item).pinned, "the last toggle")
    }

    @Test func nothingIsWrittenBeforeTheDelay() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        model.flushWrites()
        let states = harness.folder.appending(path: "states.json")
        try? FileManager.default.removeItem(at: states)
        model.togglePin(try #require(model.allItems.first))
        #expect(!FileManager.default.fileExists(atPath: states.path), "not yet: more changes may follow")
        model.flushWrites()
        #expect(FileManager.default.fileExists(atPath: states.path))
    }

    @Test func aFailedWriteIsSaidThenCleared() async throws {
        let harness = Harness()
        let model = try await harness.connected()
        model.flushWrites()
        let item = try #require(model.allItems.first)
        try FileManager.default.removeItem(at: harness.folder)
        model.togglePin(item)
        model.flushWrites()
        #expect(model.storageIssues.map(\.file) == ["states.json"])
        try FileManager.default.createDirectory(at: harness.folder, withIntermediateDirectories: true)
        model.togglePin(item)
        model.flushWrites()
        #expect(model.storageIssues.isEmpty)
    }
}

import Foundation

/// Saves `JSONStore` files off the main thread, a moment after the first change. A burst of changes (Mark all as
/// done, Undo, a refresh) writes each file once, with its latest value. One serial queue, so writes land in order.
@MainActor
final class DiskWriter {
    /// Each written file's name and, when it failed, why; on the main thread, the newest result per file only.
    var onResult: (_ file: String, _ failure: String?) -> Void = { _, _ in }

    private let delay: Duration
    private let queue = DispatchQueue(label: "Remora.DiskWriter", qos: .utility)
    /// File name → how to save its latest value.
    private var pending: [String: @Sendable () throws -> Void] = [:]
    private var scheduled: Task<Void, Never>?
    /// Batches are numbered, so a result that reaches the main thread late can't replace a newer one.
    private var batches = 0
    private var delivered: [String: Int] = [:]

    init(delay: Duration = .milliseconds(300)) {
        self.delay = delay
    }

    func save<Value: Sendable>(_ value: Value, to store: JSONStore<Value>) {
        pending[store.url.lastPathComponent] = { try store.save(value) }
        guard scheduled == nil else { return }
        scheduled = Task { [weak self, delay] in
            try? await Task.sleep(for: delay)
            guard !Task.isCancelled else { return }
            self?.write()
        }
    }

    /// Hands what's pending to the queue now, without waiting for it.
    func write() {
        guard let (number, batch) = take() else { return }
        queue.async { [weak self] in
            let results = Self.run(batch)
            DispatchQueue.main.async {
                MainActor.assumeIsolated { self?.deliver(results, of: number) }
            }
        }
    }

    /// Writes what's pending and waits until every write is on disk: before quitting.
    func flush() {
        guard let (number, batch) = take() else {
            queue.sync {}
            return
        }
        deliver(queue.sync { Self.run(batch) }, of: number)
    }

    private func take() -> (Int, [String: @Sendable () throws -> Void])? {
        scheduled?.cancel()
        scheduled = nil
        let batch = pending
        pending = [:]
        guard !batch.isEmpty else { return nil }
        batches += 1
        return (batches, batch)
    }

    private nonisolated static func run(_ batch: [String: @Sendable () throws -> Void]) -> [(String, String?)] {
        batch.map { file, save in
            do {
                try save()
                return (file, nil)
            } catch {
                return (file, error.localizedDescription)
            }
        }
    }

    private func deliver(_ results: [(String, String?)], of number: Int) {
        for (file, failure) in results where number > delivered[file, default: 0] {
            delivered[file] = number
            onResult(file, failure)
        }
    }

    /// Forgets what's pending and waits for the writes in progress, so nothing lands after Erase.
    func discard() {
        scheduled?.cancel()
        scheduled = nil
        pending = [:]
        queue.sync {}
    }
}

extension Sequence where Element: Sendable {
    /// Maps elements concurrently, at most `limit` at a time, keeping the original order.
    func concurrentMap<T: Sendable>(
        limit: Int = 8,
        _ transform: @escaping @Sendable (Element) async throws -> T
    ) async throws -> [T] {
        let elements = Array(self)
        return try await withThrowingTaskGroup(of: (Int, T).self) { group in
            var results = [T?](repeating: nil, count: elements.count)
            var next = 0
            func launch() {
                guard next < elements.count else { return }
                let index = next
                let element = elements[index]
                group.addTask { (index, try await transform(element)) }
                next += 1
            }
            for _ in 0..<limit { launch() }
            while let (index, value) = try await group.next() {
                results[index] = value
                launch()
            }
            return results.compactMap { $0 }
        }
    }
}

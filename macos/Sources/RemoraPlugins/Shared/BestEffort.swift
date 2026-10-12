import Foundation
import RemoraCore

/// A part of a fetch that may fail while the rest comes through (approvals, thread replies, notifications…).
/// A failure becomes a remark, so the inbox doesn't look complete when it isn't; a rate limit is never best-effort:
/// it fails the whole fetch, so the backoff waits it out and the last items stay shown (CODE-02).
enum BestEffort {
    /// The work's result, or nil after adding a remark naming `what` (a fixed English phrase).
    static func value<T>(
        _ what: String, remarks: inout [String], _ work: () async throws -> T
    ) async throws -> T? {
        do {
            return try await work()
        } catch {
            try note(what, error, remarks: &remarks)
            return nil
        }
    }

    /// For `async let`: the outcome, to read with `unwrap` once awaited.
    static func attempt<T: Sendable>(_ work: @Sendable () async throws -> T) async -> Result<T, Error> {
        do { return .success(try await work()) } catch { return .failure(error) }
    }

    static func unwrap<T>(_ result: Result<T, Error>, _ what: String, remarks: inout [String]) throws -> T? {
        switch result {
        case .success(let value): return value
        case .failure(let error):
            try note(what, error, remarks: &remarks)
            return nil
        }
    }

    /// Adds the remark for a failed part, once per phrase; rethrows a rate limit.
    static func note(_ what: String, _ error: Error, remarks: inout [String]) throws {
        if case HTTPError.rateLimited = error { throw error }
        let remark = L("%@ couldn’t be read: %@", L(what), error.localizedDescription)
        if !remarks.contains(remark) { remarks.append(remark) }
    }
}

extension Result {
    /// The failure, if it is one.
    var error: Failure? {
        if case .failure(let error) = self { return error }
        return nil
    }
}

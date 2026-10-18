import Foundation

/// Runs a local program, for plugins built on command-line tools.
public protocol CommandRunner: Sendable {
    /// `environment` replaces the program's environment; nil keeps Remora's. `input` goes to the program's standard
    /// input: what other processes mustn't see goes there, never in `arguments`, which any process of the user can
    /// read with `ps`.
    func run(_ executable: URL, arguments: [String], environment: [String: String]?, input: Data?) async throws -> Data
}

extension CommandRunner {
    public func run(_ executable: URL, arguments: [String]) async throws -> Data {
        try await run(executable, arguments: arguments, environment: nil, input: nil)
    }

    public func run(_ executable: URL, arguments: [String], environment: [String: String]?) async throws -> Data {
        try await run(executable, arguments: arguments, environment: environment, input: nil)
    }
}

public struct ProcessCommandRunner: CommandRunner {
    public var timeout: TimeInterval

    public init(timeout: TimeInterval = 180) {
        self.timeout = timeout
    }

    public func run(_ executable: URL, arguments: [String], environment: [String: String]?, input: Data?) async throws
        -> Data
    {
        let process = Process()
        process.executableURL = executable
        process.arguments = arguments
        if let environment { process.environment = environment }
        process.currentDirectoryURL = FileManager.default.temporaryDirectory
        let stdin = input.map { _ in Pipe() }
        process.standardInput = stdin ?? FileHandle.nullDevice
        let output = Pipe()
        let errors = Pipe()
        process.standardOutput = output
        process.standardError = errors

        try process.run()
        if let stdin, let input {
            // Written apart and closed after: a prompt bigger than the pipe's buffer would otherwise wait for the
            // program to read it, while we wait for its output.
            Task.detached {
                try? stdin.fileHandleForWriting.write(contentsOf: input)
                try? stdin.fileHandleForWriting.close()
            }
        }
        // On a Dispatch timer, not a Task: the reads below block threads, and with several programs at once Swift's
        // few cooperative threads could all be waiting on them, leaving no thread to stop a program that hangs.
        let deadline = DispatchWorkItem { Self.stop(process) }
        DispatchQueue.global().asyncAfter(deadline: .now() + timeout, execute: deadline)
        defer { deadline.cancel() }

        // Both pipes are read at once: a program that fills one while we wait on the other would block. A cancelled
        // request stops the program too, rather than leaving it running.
        let (data, errorData) = try await withTaskCancellationHandler {
            async let standardOutput = Self.readAll(output.fileHandleForReading)
            async let standardError = Self.readAll(errors.fileHandleForReading)
            return try await (standardOutput, standardError)
        } onCancel: {
            Self.stop(process)
        }
        process.waitUntilExit()
        guard process.terminationReason == .exit else { throw CommandError.timedOut }
        // A failure with nothing on stdout: what the program said on stderr is the only explanation.
        if process.terminationStatus != 0, data.isEmpty {
            throw CommandError.failed(process.terminationStatus, Self.tail(errorData))
        }
        return data
    }

    /// How long a program has to quit once asked (SIGTERM) before it is killed (SIGKILL), in seconds.
    static let grace: TimeInterval = 3

    /// Reads to the end on a Dispatch thread: a blocking read never holds one of Swift's cooperative threads.
    static func readAll(_ handle: FileHandle) async throws -> Data {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global().async {
                continuation.resume(with: Result { try handle.readToEnd() ?? Data() })
            }
        }
    }

    /// Asks the program to quit, and kills it if it's still running after `grace`: one that ignores SIGTERM doesn't
    /// stay behind.
    static func stop(_ process: Process) {
        guard process.isRunning else { return }
        process.terminate()
        let pid = process.processIdentifier
        DispatchQueue.global().asyncAfter(deadline: .now() + grace) {
            if process.isRunning { kill(pid, SIGKILL) }
        }
    }

    /// The last few lines of a program's error output, short enough for an error message.
    static func tail(_ data: Data) -> String {
        let lines = String(decoding: data, as: UTF8.self).split(whereSeparator: \.isNewline).map {
            $0.trimmingCharacters(in: .whitespaces)
        }
        return String(lines.filter { !$0.isEmpty }.suffix(3).joined(separator: " ").suffix(300))
    }
}

public enum CommandError: LocalizedError, Equatable {
    case notFound(String)
    case timedOut
    /// Exit status and the end of its error output.
    case failed(Int32, String)

    public var errorDescription: String? {
        switch self {
        case .notFound(let name): L("Couldn’t find %@.", name)
        case .timedOut: L("The command took too long and was stopped.")
        case .failed(let status, let message):
            message.isEmpty ? L("The command failed (status %d).", Int(status)) : message
        }
    }
}

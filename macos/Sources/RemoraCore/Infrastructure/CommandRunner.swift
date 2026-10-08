import Foundation

/// Runs a local program, for plugins built on command-line tools.
public protocol CommandRunner: Sendable {
    func run(_ executable: URL, arguments: [String]) async throws -> Data
}

public struct ProcessCommandRunner: CommandRunner {
    public var timeout: TimeInterval

    public init(timeout: TimeInterval = 180) {
        self.timeout = timeout
    }

    public func run(_ executable: URL, arguments: [String]) async throws -> Data {
        let process = Process()
        process.executableURL = executable
        process.arguments = arguments
        process.currentDirectoryURL = FileManager.default.temporaryDirectory
        process.standardInput = FileHandle.nullDevice
        let output = Pipe()
        process.standardOutput = output
        process.standardError = Pipe()

        try process.run()
        let deadline = Task {
            try await Task.sleep(for: .seconds(timeout))
            process.terminate()
        }
        defer { deadline.cancel() }

        let data = try await Task.detached { try output.fileHandleForReading.readToEnd() ?? Data() }.value
        process.waitUntilExit()
        guard process.terminationReason == .exit else { throw CommandError.timedOut }
        return data
    }
}

public enum CommandError: LocalizedError, Equatable {
    case notFound(String)
    case timedOut

    public var errorDescription: String? {
        switch self {
        case .notFound(let name): L("Couldn’t find %@.", name)
        case .timedOut: L("The command took too long and was stopped.")
        }
    }
}

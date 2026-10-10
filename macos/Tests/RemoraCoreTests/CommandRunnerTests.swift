import Foundation
import Testing

@testable import RemoraCore

struct CommandRunnerTests {
    private let shell = URL(fileURLWithPath: "/bin/sh")

    @Test func aFailureCarriesTheEndOfItsErrorOutput() async {
        await #expect(throws: CommandError.failed(3, "first second")) {
            try await ProcessCommandRunner(timeout: 10).run(
                shell, arguments: ["-c", "echo first >&2; echo second >&2; exit 3"])
        }
    }

    /// Stderr used to go to a pipe nobody read; past its buffer, the program blocked until the timeout.
    @Test func lotsOfErrorOutputDoesNotBlock() async throws {
        let script = "head -c 300000 /dev/zero | tr '\\\\0' x >&2; echo done"
        let data = try await ProcessCommandRunner(timeout: 10).run(shell, arguments: ["-c", script])
        #expect(String(decoding: data, as: UTF8.self) == "done\n")
    }

    @Test func theEnvironmentIsPassed() async throws {
        let data = try await ProcessCommandRunner(timeout: 10).run(
            shell, arguments: ["-c", "printf %s \"$REMORA_TEST\""],
            environment: ["REMORA_TEST": "yes", "PATH": "/usr/bin:/bin"])
        #expect(String(decoding: data, as: UTF8.self) == "yes")
    }

    /// What mustn't show in `ps` reaches the program on standard input, even past the pipe's buffer.
    @Test func inputGoesToStandardInput() async throws {
        let secret = Data("Fix the CSV export for Alice".utf8)
        let echoed = try await ProcessCommandRunner(timeout: 10).run(
            URL(fileURLWithPath: "/bin/cat"), arguments: [], environment: nil, input: secret)
        #expect(echoed == secret)
        let big = Data(repeating: UInt8(ascii: "x"), count: 300_000)
        let counted = try await ProcessCommandRunner(timeout: 10).run(
            shell, arguments: ["-c", "wc -c | tr -d ' '"], environment: nil, input: big)
        #expect(String(decoding: counted, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines) == "300000")
    }

    @Test func outputWinsOverAFailingStatus() async throws {
        // claude --output-format json prints its error as JSON and exits non-zero: that JSON is the answer to decode.
        let data = try await ProcessCommandRunner(timeout: 10).run(
            shell, arguments: ["-c", "echo '{\"is_error\": true}'; exit 1"])
        #expect(!data.isEmpty)
    }
}

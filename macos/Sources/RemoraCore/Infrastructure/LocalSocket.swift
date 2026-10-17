import Darwin
import Foundation

/// Where the `remora` command (IDEA-22) and the app meet: a Unix socket in this user's temporary folder (0700, this
/// user's alone), itself 0600, and a client must be this user. Nothing listens on the network. The rules are
/// `RemoraCommand`'s; this is both ends of the socket. Windows and Linux have theirs (`local_api.rs`), with the same
/// protocol.
public enum LocalSocket {
    /// Overrides where the app listens and the command asks (tests, a demo run).
    public static let variable = "REMORA_SOCKET"

    public struct Failure: Error, CustomStringConvertible {
        public let call: String
        public let code: Int32
        public var description: String { "\(call): \(String(cString: strerror(code)))" }
    }

    /// `REMORA_SOCKET` when set, else `remora.sock` in this user's temporary folder. Not Application Support: Erase
    /// empties that folder.
    public static func path(environment: [String: String] = ProcessInfo.processInfo.environment) -> String {
        if let path = environment[variable], !path.isEmpty { return path }
        var buffer = [CChar](repeating: 0, count: Int(PATH_MAX))
        let length = confstr(_CS_DARWIN_USER_TEMP_DIR, &buffer, buffer.count)
        let folder =
            length > 0
            ? buffer.withUnsafeBufferPointer { String(cString: $0.baseAddress!) } : NSTemporaryDirectory()
        return URL(fileURLWithPath: folder, isDirectory: true).appending(path: "remora.sock").path
    }

    /// The command line's whole run: asks the app, prints its answer. 0 done, 1 refused (by the app, or by the
    /// organization's `AllowLocalAPI`), 2 a mistake in the command, 3 Remora isn't running.
    public static func run(_ args: [String]) -> Int32 {
        if args.first == "help" || args.first == "--help" {
            print(RemoraCommand.usage)
            return 0
        }
        guard LocalAPIPolicy.allowed() else {
            FileHandle.standardError.write(Data((McpServer.disabled + "\n").utf8))
            return 1
        }
        guard let invocation = RemoraCommand.invocation(args) else {
            FileHandle.standardError.write(Data((RemoraCommand.usage + "\n").utf8))
            return 2
        }
        do {
            let (out, err, code) = RemoraCommand.output(try ask(invocation.request, at: path()), json: invocation.json)
            if !out.isEmpty { print(out) }
            if !err.isEmpty { FileHandle.standardError.write(Data((err + "\n").utf8)) }
            return code
        } catch {
            FileHandle.standardError.write(Data("Remora isn't running (\(error)).\n".utf8))
            return 3
        }
    }

    /// Sends one request line, returns the answer line.
    public static func ask(_ request: String, at path: String) throws -> String {
        let socket = try open()
        defer { close(socket) }
        try withAddress(path) { address, size in
            guard Darwin.connect(socket, address, size) == 0 else { throw Failure(call: "connect", code: errno) }
        }
        timeout(socket, seconds: 5)
        try send(request + "\n", to: socket)
        return receive(from: socket)
    }

    static func open() throws -> Int32 {
        let socket = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
        guard socket >= 0 else { throw Failure(call: "socket", code: errno) }
        var on: Int32 = 1
        setsockopt(socket, SOL_SOCKET, SO_NOSIGPIPE, &on, socklen_t(MemoryLayout<Int32>.size))
        return socket
    }

    static func withAddress<T>(_ path: String, _ body: (UnsafePointer<sockaddr>, socklen_t) throws -> T) throws -> T {
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let bytes = Array(path.utf8)
        guard bytes.count < MemoryLayout.size(ofValue: address.sun_path) else {
            throw Failure(call: "path", code: ENAMETOOLONG)
        }
        withUnsafeMutableBytes(of: &address.sun_path) { raw in
            raw.copyBytes(from: bytes)
            raw[bytes.count] = 0
        }
        let size = socklen_t(MemoryLayout<sockaddr_un>.size)
        address.sun_len = UInt8(size)
        return try withUnsafePointer(to: &address) {
            try $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { try body($0, size) }
        }
    }

    static func timeout(_ socket: Int32, seconds: Int) {
        var time = timeval(tv_sec: seconds, tv_usec: 0)
        setsockopt(socket, SOL_SOCKET, SO_RCVTIMEO, &time, socklen_t(MemoryLayout<timeval>.size))
        setsockopt(socket, SOL_SOCKET, SO_SNDTIMEO, &time, socklen_t(MemoryLayout<timeval>.size))
    }

    static func send(_ text: String, to socket: Int32) throws {
        let bytes = Array(text.utf8)
        var sent = 0
        while sent < bytes.count {
            let count = bytes[sent...].withUnsafeBytes { write(socket, $0.baseAddress, $0.count) }
            guard count > 0 else { throw Failure(call: "write", code: errno) }
            sent += count
        }
    }

    /// One line (without its newline), at most `limit` bytes; what came when the other end stops or is too slow.
    static func receive(from socket: Int32, limit: Int = 1 << 24) -> String {
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while data.count <= limit, !data.contains(UInt8(ascii: "\n")) {
            let count = read(socket, &buffer, buffer.count)
            guard count > 0 else { break }
            data.append(contentsOf: buffer[..<count])
        }
        let line = data.split(separator: UInt8(ascii: "\n"), omittingEmptySubsequences: false).first ?? Data()
        return String(decoding: line, as: UTF8.self)
    }
}

/// The app's end: listens at `path` on a thread of its own, `handler` answering each request line.
public final class LocalSocketServer: Sendable {
    public let path: String
    private let socket: Int32

    /// Fails when another app already answers there.
    public init(path: String, handler: @escaping @Sendable (String) -> String) throws {
        self.path = path
        if (try? LocalSocket.ask(#"{"command":"count"}"#, at: path)) != nil {
            throw LocalSocket.Failure(call: "bind", code: EADDRINUSE)
        }
        // A socket left by a run that ended abruptly; never another kind of file.
        var status = stat()
        if lstat(path, &status) == 0, status.st_mode & S_IFMT == S_IFSOCK { unlink(path) }
        let socket = try LocalSocket.open()
        do {
            try LocalSocket.withAddress(path) { address, size in
                guard bind(socket, address, size) == 0 else { throw LocalSocket.Failure(call: "bind", code: errno) }
            }
            guard chmod(path, 0o600) == 0 else { throw LocalSocket.Failure(call: "chmod", code: errno) }
            guard listen(socket, 8) == 0 else { throw LocalSocket.Failure(call: "listen", code: errno) }
        } catch {
            close(socket)
            throw error
        }
        self.socket = socket
        let thread = Thread {
            while true {
                let client = accept(socket, nil, nil)
                if client < 0 {
                    if errno == EINTR || errno == ECONNABORTED { continue }
                    return
                }
                Self.answer(client, handler: handler)
            }
        }
        thread.name = "remora-command"
        thread.start()
    }

    /// One request, from this user only; a client that sends nothing is dropped after a second.
    private static func answer(_ client: Int32, handler: (String) -> String) {
        defer { close(client) }
        var on: Int32 = 1
        setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, &on, socklen_t(MemoryLayout<Int32>.size))
        var uid: uid_t = 0
        var gid: gid_t = 0
        guard getpeereid(client, &uid, &gid) == 0, uid == getuid() else { return }
        LocalSocket.timeout(client, seconds: 1)
        let line = LocalSocket.receive(from: client, limit: RemoraCommand.lineLimit + 1)
        try? LocalSocket.send(handler(line) + "\n", to: client)
    }

    /// Stops listening and removes the socket (when the app quits).
    public func stop() {
        shutdown(socket, SHUT_RDWR)
        close(socket)
        unlink(path)
    }
}

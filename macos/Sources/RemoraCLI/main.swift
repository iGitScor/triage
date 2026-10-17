import Foundation
import RemoraCore

// `remora count`, `remora list --tab waiting --json`, `remora remind "Call Erin" --at 15:00` (IDEA-22): asks the Remora
// app running on this Mac, over its socket, and prints the answer. `remora mcp` (V15-01) serves the agents on this
// Mac instead. The app bundles it as Contents/Helpers/remora.
let arguments = Array(CommandLine.arguments.dropFirst())
if arguments.first == "mcp" {
    // The app's version, from the bundle this helper lives in (Contents/Helpers/remora).
    let contents = URL(fileURLWithPath: CommandLine.arguments[0]).resolvingSymlinksInPath()
        .deletingLastPathComponent().deletingLastPathComponent()
    let info = NSDictionary(contentsOf: contents.appending(path: "Info.plist"))
    let version = info?["CFBundleShortVersionString"] as? String ?? "development"
    exit(McpServer.run(version: version, allowed: LocalAPIPolicy.allowed()))
}
exit(LocalSocket.run(arguments))

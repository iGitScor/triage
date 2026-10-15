import Foundation
import RemoraCore

// `remora count`, `remora list --tab waiting --json`, `remora remind "Call Erin" --at 15:00` (IDEA-22): asks the Remora
// app running on this Mac, over its socket, and prints the answer. The app bundles it as Contents/Helpers/remora.
exit(LocalSocket.run(Array(CommandLine.arguments.dropFirst())))

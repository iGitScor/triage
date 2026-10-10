// Creates the Ed25519 key that signs Mac updates, once.
// The public key goes in Resources/Info.plist (RemoraUpdatePublicKey); the private key is written to a folder
// (default ~/Remora Update Key) for the UPDATE_SIGNING_KEY secret. Whoever has it can ship an update to every Mac
// that turned updates on: keep it offline. Replacing it means users of older versions update by hand once.
//   swift scripts/updates/make-key.swift [folder]
import CryptoKit
import Foundation

let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
    .deletingLastPathComponent()
let plist = root.appending(path: "Resources/Info.plist")
let folder =
    CommandLine.arguments.count > 1
    ? URL(fileURLWithPath: CommandLine.arguments[1])
    : FileManager.default.homeDirectoryForCurrentUser.appending(path: "Remora Update Key")
let file = folder.appending(path: "private-key.txt")

guard let info = NSDictionary(contentsOf: plist) as? [String: Any],
    var text = try? String(contentsOf: plist, encoding: .utf8),
    let end = text.range(of: "</dict>", options: .backwards)
else {
    print("Can't read \(plist.path)")
    exit(1)
}
if info["RemoraUpdatePublicKey"] != nil {
    print(
        "Info.plist already has an update key. Replacing it cuts every installed copy off from updates: remove it by hand first if you mean it."
    )
    exit(1)
}
if FileManager.default.fileExists(atPath: file.path) {
    print("\(file.path) already exists.")
    exit(1)
}

let key = Curve25519.Signing.PrivateKey()
try FileManager.default.createDirectory(
    at: folder, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
FileManager.default.createFile(
    atPath: file.path, contents: Data(key.rawRepresentation.base64EncodedString().utf8),
    attributes: [.posixPermissions: 0o600])
// One line in the file's own layout, rather than rewriting the whole plist.
let line =
    "    <key>RemoraUpdatePublicKey</key><string>\(key.publicKey.rawRepresentation.base64EncodedString())</string>\n"
text.insert(contentsOf: line, at: end.lowerBound)
try text.write(to: plist, atomically: true, encoding: .utf8)

print("Public key written to Resources/Info.plist; private key in \(file.path). Give it to the release workflow:")
print("  gh secret set UPDATE_SIGNING_KEY < \"\(file.path)\"")

// Prints the base64 Ed25519 signature of a file, with the private key from UPDATE_SIGNING_KEY (base64, 32 bytes).
// Used by the release workflow for Remora.dmg; the app checks it with the public key in its Info.plist.
//   UPDATE_SIGNING_KEY=… swift scripts/updates/sign.swift build/Remora.dmg
import CryptoKit
import Foundation

let arguments = CommandLine.arguments
guard arguments.count == 2, let encoded = ProcessInfo.processInfo.environment["UPDATE_SIGNING_KEY"],
      let raw = Data(base64Encoded: encoded.trimmingCharacters(in: .whitespacesAndNewlines)),
      let key = try? Curve25519.Signing.PrivateKey(rawRepresentation: raw) else {
    FileHandle.standardError.write(Data("usage: UPDATE_SIGNING_KEY=<base64> swift sign.swift <file>\n".utf8))
    exit(1)
}
let file = try Data(contentsOf: URL(fileURLWithPath: arguments[1]), options: .mappedIfSafe)
let signature = try key.signature(for: file)
// Check it before anyone downloads it: what the app will do.
guard key.publicKey.isValidSignature(signature, for: file) else { exit(1) }
print(signature.base64EncodedString())

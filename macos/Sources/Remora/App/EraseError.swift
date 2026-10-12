import Foundation
import RemoraCore

/// Erase cleared what it could, but files with message content are still on disk: the user deletes them by hand.
struct EraseError: LocalizedError, Equatable {
    /// The names left in the folder.
    var files: [String]
    var folder: String
    /// Why the Keychain refused too, if it did.
    var keychain: String?

    var errorDescription: String? {
        let files = L(
            "Some files couldn’t be erased: %@. Delete them from %@.", files.formatted(.list(type: .and)), folder)
        return [files, keychain].compactMap { $0 }.joined(separator: " ")
    }
}

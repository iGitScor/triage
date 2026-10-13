import Foundation

extension String {
    /// Nil for an empty string: for optional settings and fallbacks (`name.nilIfEmpty ?? default`).
    public var nilIfEmpty: String? { isEmpty ? nil : self }
}

import Foundation

/// The app's translation of an English string, from `Localizable.strings` in the app bundle.
/// Falls back to the English key (tests, previews, strings without a translation).
public func L(_ key: String) -> String {
    Bundle.main.localizedString(forKey: key, value: key, table: nil)
}

/// Translates a format string, then fills it in (`L("in %d min", 5)`).
public func L(_ key: String, _ arguments: CVarArg...) -> String {
    String(format: L(key), locale: .current, arguments: arguments)
}

public enum AppLanguage {
    /// The language the app runs in, among those it ships (English by default).
    public static var isFrench: Bool { Bundle.main.preferredLocalizations.first == "fr" }
}

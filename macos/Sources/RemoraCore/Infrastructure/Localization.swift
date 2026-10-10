import Foundation

/// The app's translation of an English string, from `Localizable.strings` in the app bundle.
/// Falls back to the English key (tests, previews, strings without a translation).
// swift-format-ignore: AlwaysUseLowerCamelCase
public func L(_ key: String) -> String {
    Bundle.main.localizedString(forKey: key, value: key, table: nil)
}

/// Translates a format string, then fills it in (`L("in %d min", 5)`).
// swift-format-ignore: AlwaysUseLowerCamelCase
public func L(_ key: String, _ arguments: CVarArg...) -> String {
    String(format: L(key), locale: .current, arguments: arguments)
}

/// A count with the right form: `singular` for one, `plural` otherwise, by the language's rule (French also
/// says "0 élément"). Both keys are translated, so each language can word them its own way.
// swift-format-ignore: AlwaysUseLowerCamelCase
public func L(_ singular: String, plural: String, _ count: Int) -> String {
    L(AppLanguage.isSingular(count) ? singular : plural, count)
}

public enum AppLanguage {
    /// One in English; zero and one in French.
    public static func isSingular(_ count: Int, french: Bool = isFrench) -> Bool {
        french ? abs(count) < 2 : count == 1
    }

    /// The language the app runs in, among those it ships (English by default).
    public static var isFrench: Bool { Bundle.main.preferredLocalizations.first == "fr" }
}

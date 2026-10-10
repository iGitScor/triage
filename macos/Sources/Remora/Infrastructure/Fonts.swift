import AppKit
import CoreText
import SwiftUI

/// Loads Myna's Outfit variable font and instantiates it at any weight.
enum Fonts {
    /// Loaded and registered once, on first use: a `static let` is initialized exactly once, thread-safely, and a
    /// font descriptor is immutable, so sharing it is safe.
    nonisolated(unsafe) private static let descriptor: CTFontDescriptor? = {
        guard let url = Bundle.main.url(forResource: "Outfit", withExtension: "woff2"),
              let descriptors = CTFontManagerCreateFontDescriptorsFromURL(url as CFURL) as? [CTFontDescriptor] else { return nil }
        CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
        return descriptors.first
    }()
    private static let weightAxis = 0x7767_6874 // "wght"

    static var hasOutfit: Bool { descriptor != nil }

    /// At launch, so the first view doesn't pay for it.
    static func register() {
        _ = descriptor
    }

    static func outfit(size: CGFloat, weight: Font.Weight) -> Font? {
        nsOutfit(size: size, weight: weight).map { Font($0) }
    }

    static func nsOutfit(size: CGFloat, weight: Font.Weight) -> NSFont? {
        guard let descriptor else { return nil }
        let variation = [weightAxis: value(of: weight)] as CFDictionary
        let attributes = [kCTFontVariationAttribute: variation] as CFDictionary
        return CTFontCreateWithFontDescriptor(CTFontDescriptorCreateCopyWithAttributes(descriptor, attributes), size, nil) as NSFont
    }

    private static func value(of weight: Font.Weight) -> Double {
        switch weight {
        case .ultraLight: 200
        case .thin: 100
        case .light: 300
        case .medium: 500
        case .semibold: 600
        case .bold: 700
        case .heavy: 800
        case .black: 900
        default: 400
        }
    }
}

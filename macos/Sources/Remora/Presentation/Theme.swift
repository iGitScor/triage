import AppKit
import RemoraCore
import SwiftUI

/// Myna design tokens (see podcast/brand/tokens.css), resolved for light and dark appearance.
enum Myna {
    // Each token has a high-contrast variant, used when Increase Contrast is on (UI-04):
    // darker inks and borders on light, lighter ones on dark, and stronger hairlines.
    static let backdrop = Color(light: 0xF0F0E8, dark: 0x0E0E14)
    static let surface = Color(light: 0xF7F7F1, dark: 0x13131C, contrast: (0xFFFFFF, 0x000000))
    static let card = Color(light: 0xE8E8DF, dark: 0x1A1A26)
    static let card2 = Color(light: 0xDCDCD2, dark: 0x242433, contrast: (0xCFCFC4, 0x2E2E40))
    static let field = Color(light: 0xFFFFFF, dark: 0x13131C, contrast: (0xFFFFFF, 0x000000))
    static let border = Color(light: 0xD0D0C8, dark: 0x2A2A3A, contrast: (0x6E6E66, 0x8C8CA2))
    static let ink = Color(light: 0x111111, dark: 0xE4E4EC, contrast: (0x000000, 0xFFFFFF))
    static let inkSoft = Color(light: 0x333330, dark: 0xC4C4D0, contrast: (0x111111, 0xF0F0F6))
    static let muted = Color(light: 0x55554E, dark: 0x8C8CA2, contrast: (0x2E2E2A, 0xC4C4D0))
    static let selection = Color(
        light: 0x111111, dark: 0xE4E4EC, opacity: 0.6, contrast: (0x000000, 0xFFFFFF), contrastOpacity: 1
    )
    static let line = Color(light: 0x111111, dark: 0xFFFFFF, opacity: 0.08, contrastOpacity: 0.35)
    static let accent = Color(light: 0xB9FF66, dark: 0xB9FF66)
    static let accentDeep = Color(light: 0x9DE049, dark: 0x9DE049, contrast: (0x6FA82A, 0xB9FF66))
    static let accentSoft = Color(
        light: 0xB9FF66, dark: 0xB9FF66, lightOpacity: 0.35, darkOpacity: 0.14, contrastOpacity: (0.6, 0.3)
    )
    static let accentText = Color(light: 0x356509, dark: 0xB9FF66, contrast: (0x234504, 0xB9FF66))
    static let onAccent = Color(light: 0x111111, dark: 0x111111, contrast: (0x000000, 0x000000))
    static let dark = Color(light: 0x111111, dark: 0x07070B, contrast: (0x000000, 0x000000))
    static let onDark = Color(light: 0xF0F0E8, dark: 0xF0F0E8, contrast: (0xFFFFFF, 0xFFFFFF))
    static let ok = Color(light: 0x146B50, dark: 0x34D399, contrast: (0x0B4F3A, 0x6EE7B7))
    static let warn = Color(light: 0x725200, dark: 0xFBBF24, contrast: (0x553D00, 0xFCD34D))
    static let danger = Color(light: 0xA82C23, dark: 0xF87171, contrast: (0x861C15, 0xFCA5A5))

    /// Secondary text on `dark` surfaces: translucent, but nearly opaque with Increase Contrast.
    static func onDark(opacity: Double) -> Color {
        Color(light: 0xF0F0E8, dark: 0xF0F0E8, opacity: opacity, contrast: (0xFFFFFF, 0xFFFFFF), contrastOpacity: max(opacity, 0.9))
    }

    static let radiusLarge: CGFloat = 18
    static let radiusMedium: CGFloat = 12

    /// No text under 11 pt, and every size follows the text size chosen in Settings → General.
    static let smallestText: CGFloat = 11
    @MainActor static var textScale: CGFloat = 1

    @MainActor static func font(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font {
        let size = max(smallestText, size) * textScale
        return Fonts.outfit(size: size, weight: weight) ?? .system(size: size, weight: weight, design: .rounded)
    }

    static func color(for tone: Tone) -> (foreground: Color, background: Color) {
        switch tone {
        case .accent: (onAccent, accent)
        case .positive: (ok, ok.opacity(0.14))
        case .negative: (danger, danger.opacity(0.14))
        case .warning: (warn, warn.opacity(0.16))
        case .neutral: (muted, line)
        }
    }
}

extension Color {
    /// `contrast` holds the (light, dark) values used with Increase Contrast; they default to the regular ones.
    init(
        light: UInt32, dark: UInt32, opacity: Double = 1,
        contrast: (light: UInt32, dark: UInt32)? = nil, contrastOpacity: Double? = nil
    ) {
        let contrastOpacity = contrastOpacity ?? opacity
        self.init(
            light: light, dark: dark, lightOpacity: opacity, darkOpacity: opacity,
            contrast: contrast, contrastOpacity: (contrastOpacity, contrastOpacity)
        )
    }

    init(
        light: UInt32, dark: UInt32, lightOpacity: Double, darkOpacity: Double,
        contrast: (light: UInt32, dark: UInt32)? = nil, contrastOpacity: (light: Double, dark: Double)? = nil
    ) {
        let contrast = contrast ?? (light, dark)
        let contrastOpacity = contrastOpacity ?? (lightOpacity, darkOpacity)
        self.init(nsColor: NSColor(name: nil) { appearance in
            let isDark = appearance.isDark
            guard appearance.isHighContrast else {
                return NSColor(hex: isDark ? dark : light, alpha: isDark ? darkOpacity : lightOpacity)
            }
            return NSColor(
                hex: isDark ? contrast.dark : contrast.light,
                alpha: isDark ? contrastOpacity.dark : contrastOpacity.light
            )
        })
    }

    /// The token resolved for AppKit drawing, which follows the app's appearance rather than the view's.
    @MainActor func resolved(dark: Bool) -> NSColor {
        var resolved = NSColor.clear
        NSAppearance(named: dark ? .darkAqua : .aqua)?.performAsCurrentDrawingAppearance {
            resolved = NSColor(cgColor: NSColor(self).cgColor) ?? .clear
        }
        return resolved
    }
}

extension NSAppearance {
    private static let names: [NSAppearance.Name] = [
        .aqua, .darkAqua, .accessibilityHighContrastAqua, .accessibilityHighContrastDarkAqua,
    ]

    var isDark: Bool {
        let match = bestMatch(from: Self.names)
        return match == .darkAqua || match == .accessibilityHighContrastDarkAqua
    }

    /// Stands in for Increase Contrast in tests.
    nonisolated(unsafe) static var increaseContrastOverride: Bool?

    /// Increase Contrast. Checked on the workspace too, since an appearance built by name never reports it.
    var isHighContrast: Bool {
        if let override = Self.increaseContrastOverride { return override }
        let match = bestMatch(from: Self.names)
        return match == .accessibilityHighContrastAqua || match == .accessibilityHighContrastDarkAqua
            || NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast
    }
}

extension NSColor {
    convenience init(hex: UInt32, alpha: Double = 1) {
        self.init(
            srgbRed: CGFloat((hex >> 16) & 0xFF) / 255,
            green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255,
            alpha: alpha
        )
    }
}

extension Preferences.Appearance {
    /// For AppKit drawing that can't use dynamic colors.
    @MainActor var isDark: Bool {
        switch self {
        case .light: false
        case .dark: true
        case .system: NSApp.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        }
    }

    var colorScheme: ColorScheme? {
        switch self {
        case .system: nil
        case .light: .light
        case .dark: .dark
        }
    }
}

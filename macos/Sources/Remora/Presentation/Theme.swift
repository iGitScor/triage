import AppKit
import RemoraCore
import SwiftUI

/// Myna design tokens (see podcast/brand/tokens.css), resolved for light and dark appearance.
enum Myna {
    static let backdrop = Color(light: 0xF0F0E8, dark: 0x0E0E14)
    static let surface = Color(light: 0xF7F7F1, dark: 0x13131C)
    static let card = Color(light: 0xE8E8DF, dark: 0x1A1A26)
    static let card2 = Color(light: 0xDCDCD2, dark: 0x242433)
    static let field = Color(light: 0xFFFFFF, dark: 0x13131C)
    static let border = Color(light: 0xD0D0C8, dark: 0x2A2A3A)
    static let ink = Color(light: 0x111111, dark: 0xE4E4EC)
    static let inkSoft = Color(light: 0x333330, dark: 0xC4C4D0)
    static let muted = Color(light: 0x55554E, dark: 0x8C8CA2)
    static let line = Color(light: 0x111111, dark: 0xFFFFFF, opacity: 0.08)
    static let accent = Color(light: 0xB9FF66, dark: 0xB9FF66)
    static let accentDeep = Color(light: 0x9DE049, dark: 0x9DE049)
    static let accentSoft = Color(light: 0xB9FF66, dark: 0xB9FF66, lightOpacity: 0.35, darkOpacity: 0.14)
    static let accentText = Color(light: 0x356509, dark: 0xB9FF66)
    static let onAccent = Color(light: 0x111111, dark: 0x111111)
    static let dark = Color(light: 0x111111, dark: 0x07070B)
    static let onDark = Color(light: 0xF0F0E8, dark: 0xF0F0E8)
    static let ok = Color(light: 0x146B50, dark: 0x34D399)
    static let warn = Color(light: 0x725200, dark: 0xFBBF24)
    static let danger = Color(light: 0xA82C23, dark: 0xF87171)

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
    init(light: UInt32, dark: UInt32, opacity: Double = 1) {
        self.init(light: light, dark: dark, lightOpacity: opacity, darkOpacity: opacity)
    }

    init(light: UInt32, dark: UInt32, lightOpacity: Double, darkOpacity: Double) {
        self.init(nsColor: NSColor(name: nil) { appearance in
            let isDark = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            return NSColor(hex: isDark ? dark : light, alpha: isDark ? darkOpacity : lightOpacity)
        })
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

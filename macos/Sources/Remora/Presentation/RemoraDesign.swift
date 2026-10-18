import AppKit
import RemoraCore
import SwiftUI

/// Remora's look: the Estuary design system's colours (EstuaryTokens.swift, made from @iscor/design by
/// scripts/tokens-check.py), and what is the Mac app's own: its sizes, its fonts, the selection.
enum RemoraDesign {
    /// The selection: ink at 60 %, opaque with Increase Contrast. Remora's own, not a design system token.
    static let selection = translucent(ink, opacity: 0.6, contrastOpacity: 1)

    /// Secondary text on `dark` surfaces: translucent, but nearly opaque with Increase Contrast.
    static func onDark(opacity: Double) -> Color {
        translucent(onDark, opacity: opacity, contrastOpacity: max(opacity, 0.9))
    }

    /// A token at another opacity, which Increase Contrast raises.
    private static func translucent(_ token: Color, opacity: Double, contrastOpacity: Double) -> Color {
        let base = NSColor(token)
        return Color(
            nsColor: NSColor(name: nil) { appearance in
                var resolved = base
                appearance.performAsCurrentDrawingAppearance { resolved = NSColor(cgColor: base.cgColor) ?? base }
                return resolved.withAlphaComponent(appearance.isHighContrast ? contrastOpacity : opacity)
            })
    }

    static let radiusLarge: CGFloat = 18
    static let radiusMedium: CGFloat = 12

    /// No text under 11 pt, and every size follows the text size chosen in Settings → General.
    static let smallestText: CGFloat = 11
    @MainActor static var textScale: CGFloat = 1

    /// A size as shown: never under 11 pt, then scaled.
    @MainActor static func textSize(_ size: CGFloat) -> CGFloat { max(smallestText, size) * textScale }

    @MainActor static func font(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font {
        let size = textSize(size)
        return Fonts.outfit(size: size, weight: weight) ?? .system(size: size, weight: weight, design: .rounded)
    }

    /// The system's monospaced (paths, line counts) or rounded (key caps) face, at the same size as `font`.
    @MainActor static func system(
        _ size: CGFloat, _ weight: Font.Weight = .regular, design: Font.Design = .monospaced
    ) -> Font {
        .system(size: textSize(size), weight: weight, design: design)
    }

    static func color(for tone: Tone) -> (foreground: Color, background: Color) {
        switch tone {
        case .accent: (onAccent, accent)
        case .positive: (ok, okSoft)
        case .negative: (danger, dangerSoft)
        case .warning: (warn, warnSoft)
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
        self.init(
            nsColor: NSColor(name: nil) { appearance in
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

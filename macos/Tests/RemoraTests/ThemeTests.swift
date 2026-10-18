import AppKit
import SwiftUI
import Testing

@testable import Remora

/// Tokens follow Increase Contrast.
@MainActor
@Suite(.serialized)
struct ThemeTests {
    private func luminance(_ color: NSColor) -> Double {
        let c = color.usingColorSpace(.sRGB)!
        func linear(_ v: CGFloat) -> Double { v <= 0.03928 ? v / 12.92 : pow((v + 0.055) / 1.055, 2.4) }
        return 0.2126 * linear(c.redComponent) + 0.7152 * linear(c.greenComponent) + 0.0722 * linear(c.blueComponent)
    }

    private func ratio(_ a: NSColor, _ b: NSColor) -> Double {
        let (l1, l2) = (luminance(a), luminance(b))
        return (max(l1, l2) + 0.05) / (min(l1, l2) + 0.05)
    }

    private func resolve(_ color: Color, dark: Bool, contrast: Bool) -> NSColor {
        NSAppearance.increaseContrastOverride = contrast
        defer { NSAppearance.increaseContrastOverride = nil }
        return color.resolved(dark: dark)
    }

    @Test func regularTokensKeepTheirValues() {
        #expect(resolve(Myna.ink, dark: false, contrast: false) == NSColor(hex: 0x111111))
        #expect(resolve(Myna.ink, dark: true, contrast: false) == NSColor(hex: 0xE4E4EC))
        #expect(resolve(Myna.border, dark: true, contrast: false) == NSColor(hex: 0x2A2A3A))
    }

    @Test(arguments: [false, true])
    func increaseContrastStrengthensTextAndBorders(dark: Bool) {
        let surface = resolve(Myna.surface, dark: dark, contrast: false)
        let strong = resolve(Myna.surface, dark: dark, contrast: true)
        for token in [
            Myna.ink, Myna.inkSoft, Myna.muted, Myna.border, Myna.accentText, Myna.ok, Myna.warn, Myna.danger,
        ] {
            let regular = ratio(resolve(token, dark: dark, contrast: false), surface)
            let high = ratio(resolve(token, dark: dark, contrast: true), strong)
            #expect(high >= regular)
        }
        #expect(
            resolve(Myna.line, dark: dark, contrast: true).alphaComponent
                > resolve(Myna.line, dark: dark, contrast: false).alphaComponent)
        #expect(resolve(Myna.onDark(opacity: 0.5), dark: dark, contrast: true).alphaComponent >= 0.9)
    }

    /// Every text size, the monospaced and rounded ones too, keeps the 11 pt floor and follows Settings → General.
    /// The selection and the dark bands' secondary text, derived from the generated tokens, keep their values.
    @Test func derivedColoursKeepTheirValues() {
        let selection = resolve(Myna.selection, dark: false, contrast: false)
        #expect(selection.usingColorSpace(.sRGB) == NSColor(hex: 0x111111, alpha: 0.6))
        #expect(resolve(Myna.selection, dark: true, contrast: true).usingColorSpace(.sRGB) == NSColor(hex: 0xFFFFFF))
        #expect(
            resolve(Myna.onDark(opacity: 0.5), dark: true, contrast: false).usingColorSpace(.sRGB)
                == NSColor(hex: 0xF0F0E8, alpha: 0.5))
        #expect(
            resolve(Myna.onDark(opacity: 0.5), dark: false, contrast: true).usingColorSpace(.sRGB)
                == NSColor(hex: 0xFFFFFF, alpha: 0.9))
    }

    @Test func textSizesFollowTheSetting() {
        defer { Myna.textScale = 1 }
        #expect(Myna.textSize(9) == 11 && Myna.textSize(13) == 13)
        Myna.textScale = 1.3
        #expect(abs(Myna.textSize(11) - 14.3) < 0.001)
        #expect(abs(Myna.textSize(8) - 14.3) < 0.001, "the floor applies before scaling")
    }

    /// The undo bar is dark in both themes, so its lime Undo and its message read well.
    @Test(arguments: [false, true])
    func theUndoBarReadsInBothThemes(dark: Bool) {
        let bar = resolve(Myna.dark, dark: dark, contrast: false)
        #expect(ratio(resolve(Myna.accent, dark: dark, contrast: false), bar) >= 4.5)
        #expect(ratio(resolve(Myna.onDark, dark: dark, contrast: false), bar) >= 4.5)
    }
}

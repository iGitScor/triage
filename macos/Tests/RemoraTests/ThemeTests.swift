import AppKit
import SwiftUI
import Testing
@testable import Remora

/// UI-04: tokens follow Increase Contrast.
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
        for token in [Myna.ink, Myna.inkSoft, Myna.muted, Myna.border, Myna.accentText, Myna.ok, Myna.warn, Myna.danger] {
            let regular = ratio(resolve(token, dark: dark, contrast: false), surface)
            let high = ratio(resolve(token, dark: dark, contrast: true), strong)
            #expect(high >= regular)
        }
        #expect(resolve(Myna.line, dark: dark, contrast: true).alphaComponent > resolve(Myna.line, dark: dark, contrast: false).alphaComponent)
        #expect(resolve(Myna.onDark(opacity: 0.5), dark: dark, contrast: true).alphaComponent >= 0.9)
    }
}

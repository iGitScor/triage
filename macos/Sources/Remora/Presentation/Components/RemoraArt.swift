import AppKit

/// The Remora mark: a simple fish. The name tells the remora story; the mark stays readable at 13 pt.
/// Drawn in code so the menu bar, the header and the app icon share one shape (scripts/make-icon.swift uses it too).
enum RemoraArt {
    /// The fish in a unit square (y up), facing right: a rounded body and a soft forked tail.
    /// Kept deliberately simple so it reads at menu bar size; parts are filled one by one.
    static func parts() -> [CGPath] {
        let body = CGPath(roundedRect: CGRect(x: 0.22, y: 0.36, width: 0.70, height: 0.26),
                          cornerWidth: 0.13, cornerHeight: 0.13, transform: nil)
        let tail = CGMutablePath()
        tail.move(to: CGPoint(x: 0.27, y: 0.49))
        tail.addCurve(to: CGPoint(x: 0.06, y: 0.64), control1: CGPoint(x: 0.18, y: 0.56), control2: CGPoint(x: 0.10, y: 0.64))
        tail.addCurve(to: CGPoint(x: 0.06, y: 0.34), control1: CGPoint(x: 0.10, y: 0.55), control2: CGPoint(x: 0.10, y: 0.43))
        tail.addCurve(to: CGPoint(x: 0.27, y: 0.49), control1: CGPoint(x: 0.10, y: 0.34), control2: CGPoint(x: 0.18, y: 0.42))
        tail.closeSubpath()
        return [body, tail]
    }

    /// The eye, cut out of the fill.
    static func details() -> CGPath {
        CGPath(ellipseIn: CGRect(x: 0.77, y: 0.48, width: 0.06, height: 0.06), transform: nil)
    }

    /// Where the fish sits in the unit square.
    static let bounds = CGRect(x: 0.05, y: 0.33, width: 0.88, height: 0.32)

    /// Draws the fish in `rect` of the current context.
    static func draw(in rect: CGRect, color: NSColor, context: CGContext) {
        context.saveGState()
        context.translateBy(x: rect.minX, y: rect.minY)
        context.scaleBy(x: rect.width, y: rect.height)
        context.beginTransparencyLayer(auxiliaryInfo: nil)
        context.setFillColor(color.cgColor)
        for part in parts() {
            context.addPath(part)
            context.fillPath()
        }
        context.setBlendMode(.clear)
        context.addPath(details())
        context.fillPath()
        context.endTransparencyLayer()
        context.restoreGState()
    }

    /// The fish alone, e.g. as a template image for the menu bar.
    static func image(size: NSSize, color: NSColor = .black, template: Bool = false) -> NSImage {
        let image = NSImage(size: size, flipped: false) { rect in
            guard let context = NSGraphicsContext.current?.cgContext else { return false }
            draw(in: rect, color: color, context: context)
            return true
        }
        image.isTemplate = template
        return image
    }

    /// The menu bar glyph: cropped to the fish so it reads at 13 pt.
    static func glyph(height: CGFloat = 13) -> NSImage {
        let image = NSImage(size: NSSize(width: height * 2, height: height), flipped: false) { rect in
            guard let context = NSGraphicsContext.current?.cgContext else { return false }
            drawCropped(in: rect, color: .black, context: context)
            return true
        }
        image.isTemplate = true
        return image
    }

    /// Draws the fish cropped to its bounds, filling `rect`.
    static func drawCropped(in rect: CGRect, color: NSColor, context: CGContext) {
        let scaleX = rect.width / bounds.width, scaleY = rect.height / bounds.height
        draw(in: CGRect(x: rect.minX - bounds.minX * scaleX, y: rect.minY - bounds.minY * scaleY, width: scaleX, height: scaleY),
             color: color, context: context)
    }

    /// The mark: the fish on a lime disc.
    static func mark(size: CGFloat, disc: NSColor, fish: NSColor) -> NSImage {
        NSImage(size: NSSize(width: size, height: size), flipped: false) { rect in
            guard let context = NSGraphicsContext.current?.cgContext else { return false }
            context.setFillColor(disc.cgColor)
            context.fillEllipse(in: rect)
            draw(in: rect.insetBy(dx: rect.width * 0.07, dy: rect.height * 0.07), color: fish, context: context)
            return true
        }
    }
}

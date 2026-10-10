import AppKit
import RemoraPlugins

/// A tool's icon: its bundled logo when there is one, else an SF Symbol. Never fetched from the network.
enum ToolIcon {
    static func image(pluginID: String, pointSize: CGFloat, color: NSColor?) -> NSImage? {
        let manifest = PluginRegistry.manifest(pluginID)
        let base: NSImage?
        if let logo = manifest?.logo,
            let url = Bundle.main.url(forResource: logo, withExtension: "svg", subdirectory: "Logos"),
            let svg = NSImage(contentsOf: url)
        {
            svg.size = NSSize(width: pointSize, height: pointSize)
            base = svg
        } else {
            base = NSImage(systemSymbolName: manifest?.symbol ?? "alarm", accessibilityDescription: manifest?.name)?
                .withSymbolConfiguration(.init(pointSize: pointSize - 1, weight: .semibold))
        }
        guard let base else { return nil }
        guard let color else {
            base.isTemplate = true
            return base
        }
        return tinted(base, color)
    }

    /// The shape of a template image filled with one color.
    private static func tinted(_ image: NSImage, _ color: NSColor) -> NSImage {
        let tinted = NSImage(size: image.size, flipped: false) { rect in
            image.draw(in: rect)
            color.set()
            rect.fill(using: .sourceAtop)
            return true
        }
        tinted.isTemplate = false
        return tinted
    }
}

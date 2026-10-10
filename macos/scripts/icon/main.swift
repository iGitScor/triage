// Renders Resources/AppIcon.icns: the remora on a lime disc, on an ink tile.
// Built with Sources/Remora/Presentation/Components/RemoraArt.swift (see the Makefile `icon` target).
import AppKit

let root = URL(fileURLWithPath: CommandLine.arguments[1])
let iconset = FileManager.default.temporaryDirectory.appending(path: "AppIcon.iconset")
try? FileManager.default.removeItem(at: iconset)
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)

func render(_ size: Int) -> Data {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8,
        samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
        bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let s = CGFloat(size)
    let tile = NSRect(x: s * 0.1, y: s * 0.1, width: s * 0.8, height: s * 0.8)
    NSColor(srgbRed: 0x11 / 255, green: 0x11 / 255, blue: 0x11 / 255, alpha: 1).setFill()
    NSBezierPath(roundedRect: tile, xRadius: s * 0.18, yRadius: s * 0.18).fill()
    let disc = tile.insetBy(dx: s * 0.12, dy: s * 0.12)
    NSColor(srgbRed: 0xB9 / 255, green: 1, blue: 0x66 / 255, alpha: 1).setFill()
    NSBezierPath(ovalIn: disc).fill()
    if let context = NSGraphicsContext.current?.cgContext {
        RemoraArt.draw(
            in: disc.insetBy(dx: disc.width * 0.05, dy: disc.height * 0.05),
            color: NSColor(srgbRed: 0x11 / 255, green: 0x11 / 255, blue: 0x11 / 255, alpha: 1), context: context)
    }
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

for base in [16, 32, 128, 256, 512] {
    try render(base).write(to: iconset.appending(path: "icon_\(base)x\(base).png"))
    try render(base * 2).write(to: iconset.appending(path: "icon_\(base)x\(base)@2x.png"))
}
let task = Process()
task.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
task.arguments = ["-c", "icns", iconset.path, "-o", root.appending(path: "Resources/AppIcon.icns").path]
try task.run()
task.waitUntilExit()

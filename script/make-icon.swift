import AppKit

// Draws the Repodeck app icon (a deck of terminal cards, the front one showing a `❯_` prompt) and
// writes macos/AppIcon.icns, which `cargo bundle` copies into the app.
// Run from the project root: swift script/make-icon.swift [preview.png]

let iconset = "target/AppIcon.iconset"
let output = "macos/AppIcon.icns"
try? FileManager.default.createDirectory(atPath: iconset, withIntermediateDirectories: true)

func rgb(_ hex: UInt32, _ alpha: CGFloat = 1) -> CGColor {
    CGColor(srgbRed: CGFloat(hex >> 16 & 0xff) / 255, green: CGFloat(hex >> 8 & 0xff) / 255,
            blue: CGFloat(hex & 0xff) / 255, alpha: alpha)
}

/// Superellipse (n = 5): the continuous-corner "squircle" of macOS app icons.
func squircle(_ r: CGRect) -> CGPath {
    let path = CGMutablePath(), n = 5.0, steps = 360
    for i in 0...steps {
        let t = Double(i) / Double(steps) * 2 * .pi
        let c = cos(t), s = sin(t)
        let x = pow(abs(c), 2 / n) * (c < 0 ? -1 : 1), y = pow(abs(s), 2 / n) * (s < 0 ? -1 : 1)
        let p = CGPoint(x: r.midX + x * r.width / 2, y: r.midY + y * r.height / 2)
        i == 0 ? path.move(to: p) : path.addLine(to: p)
    }
    path.closeSubpath()
    return path
}

/// Draws in a 1024-point space with y pointing up.
func render(_ px: Int) -> Data {
    let ctx = CGContext(data: nil, width: px, height: px, bitsPerComponent: 8, bytesPerRow: 0,
                        space: CGColorSpace(name: CGColorSpace.sRGB)!,
                        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    ctx.scaleBy(x: CGFloat(px) / 1024, y: CGFloat(px) / 1024)

    // Tile: the macOS grid's 824-point squircle with a soft drop shadow.
    let tile = CGRect(x: 100, y: 100, width: 824, height: 824)
    ctx.saveGState()
    ctx.setShadow(offset: CGSize(width: 0, height: -10), blur: 24, color: CGColor(gray: 0, alpha: 0.3))
    ctx.addPath(squircle(tile))
    ctx.setFillColor(rgb(0x1c1d22))
    ctx.fillPath()
    ctx.restoreGState()
    ctx.saveGState()
    ctx.addPath(squircle(tile))
    ctx.clip()
    let gradient = CGGradient(colorsSpace: nil, colors: [rgb(0x2d2f37), rgb(0x18191d)] as CFArray,
                              locations: [0, 1])!
    ctx.drawLinearGradient(gradient, start: CGPoint(x: 512, y: 924), end: CGPoint(x: 512, y: 100), options: [])

    // Two cards peeking out behind the front one.
    for (inset, rise, alpha) in [(118.0, 112.0, 0.28), (78.0, 58.0, 0.5)] {
        let card = CGRect(x: 190 + inset, y: 270 + rise, width: 644 - 2 * inset, height: 400)
        ctx.addPath(CGPath(roundedRect: card, cornerWidth: 44, cornerHeight: 44, transform: nil))
        ctx.setFillColor(rgb(0x4a4d58, alpha))
        ctx.fillPath()
    }

    // Front card: a terminal window.
    let front = CGRect(x: 190, y: 250, width: 644, height: 420)
    ctx.saveGState()
    ctx.setShadow(offset: CGSize(width: 0, height: -12), blur: 30, color: CGColor(gray: 0, alpha: 0.45))
    ctx.addPath(CGPath(roundedRect: front, cornerWidth: 48, cornerHeight: 48, transform: nil))
    ctx.setFillColor(rgb(0x23252b))
    ctx.fillPath()
    ctx.restoreGState()
    ctx.addPath(CGPath(roundedRect: front.insetBy(dx: 2, dy: 2), cornerWidth: 46, cornerHeight: 46, transform: nil))
    ctx.setStrokeColor(rgb(0xffffff, 0.08))
    ctx.setLineWidth(4)
    ctx.strokePath()
    for (i, color) in [0xff5f57, 0xfebc2e, 0x28c840].enumerated() {
        ctx.setFillColor(rgb(UInt32(color)))
        ctx.fillEllipse(in: CGRect(x: 240 + CGFloat(i) * 46, y: 600, width: 28, height: 28))
    }

    // `❯_` prompt: an orange chevron and a light cursor bar.
    ctx.setLineCap(.round)
    ctx.setLineJoin(.round)
    ctx.setLineWidth(46)
    ctx.setStrokeColor(rgb(0xe08a5f))
    ctx.move(to: CGPoint(x: 300, y: 520))
    ctx.addLine(to: CGPoint(x: 400, y: 440))
    ctx.addLine(to: CGPoint(x: 300, y: 360))
    ctx.strokePath()
    ctx.setStrokeColor(rgb(0xe8e8ec))
    ctx.move(to: CGPoint(x: 470, y: 352))
    ctx.addLine(to: CGPoint(x: 600, y: 352))
    ctx.strokePath()
    ctx.restoreGState()

    return NSBitmapImageRep(cgImage: ctx.makeImage()!).representation(using: .png, properties: [:])!
}

if CommandLine.arguments.count > 1 {
    try! render(1024).write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
}

let entries: [(String, Int)] = [
    ("icon_16x16", 16), ("icon_16x16@2x", 32),
    ("icon_32x32", 32), ("icon_32x32@2x", 64),
    ("icon_128x128", 128), ("icon_128x128@2x", 256),
    ("icon_256x256", 256), ("icon_256x256@2x", 512),
    ("icon_512x512", 512), ("icon_512x512@2x", 1024),
]
var cache: [Int: Data] = [:]
for (name, px) in entries {
    let data = cache[px] ?? render(px)
    cache[px] = data
    try! data.write(to: URL(fileURLWithPath: "\(iconset)/\(name).png"))
}

let iconutil = Process()
iconutil.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
iconutil.arguments = ["-c", "icns", iconset, "-o", output]
try! iconutil.run()
iconutil.waitUntilExit()
print(iconutil.terminationStatus == 0 ? "✓ \(output)" : "✗ iconutil failed (\(iconutil.terminationStatus))")
exit(iconutil.terminationStatus)

import Foundation
import Testing

@testable import Remora

/// What the menu bar shows and says. A dot instead of the number is calmer; it says something needs you, not how
/// many. As Windows and Linux's tray.
@MainActor
struct MenuBarTitleTests {
    @Test func aDotSaysSomethingNeedsYouNotHowMany() {
        let count = MenuBarTitle.Content(counts: [(nil, 3)])
        let dot = MenuBarTitle.Content(counts: [(nil, 3)], dot: true)
        #expect(MenuBarTitle.make(count).string == " 3")
        #expect(MenuBarTitle.make(dot).string == " \u{25CF}")
        #expect(MenuBarTitle.spokenText(count) == "Remora, 3 need you")
        #expect(MenuBarTitle.spokenText(dot) == "Remora, Something needs you")
        #expect(MenuBarTitle.spokenText(MenuBarTitle.Content(dot: true)) == "Remora, Nothing needs you")
    }
}

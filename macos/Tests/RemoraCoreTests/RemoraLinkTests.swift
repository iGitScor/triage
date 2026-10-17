import Foundation
import Testing

@testable import RemoraCore

/// The same cases as Windows (`link.rs`).
struct RemoraLinkTests {
    let now = Date(iso8601: "2026-10-13T12:00:00Z")!

    func link(_ text: String) -> RemoraLink? { URL(string: text).flatMap { RemoraLink($0, now: now) } }

    @Test func opensTheInboxOnATab() {
        #expect(link("remora://open") == .open(tab: nil))
        #expect(link("REMORA://open/?tab=Waiting") == .open(tab: "waiting"))
        #expect(link("remora://open?tab=in-progress") == .open(tab: "inProgress"))
        #expect(link("remora://open?tab=nowhere") == .open(tab: nil), "an unknown tab: the inbox")
    }

    @Test func fillsInTheReminderPanelWithoutSavingIt() {
        #expect(
            link("remora://remind?text=Call%20Erin+back&at=2026-10-14T09:00:00Z")
                == .remind(text: "Call Erin back", at: Date(iso8601: "2026-10-14T09:00:00Z")))
        let local = DateComponents(calendar: .current, year: 2026, month: 10, day: 14, hour: 9).date
        #expect(link("remora://remind?at=2026-10-14T09:00") == .remind(text: "", at: local))
        #expect(link("remora://remind?text=x&at=2026-10-01T09:00:00Z") == .remind(text: "x", at: nil), "already past")
        guard case .remind(let text, _) = link("remora://remind?text=" + String(repeating: "a", count: 500)) else {
            Issue.record("not a reminder")
            return
        }
        #expect(text.count == RemoraLink.textLimit)
    }

    @Test func refreshesAndRefusesTheRest() {
        #expect(link("remora://refresh") == .refresh)
        for refused in ["remora://done?id=1", "remora://snooze", "https://remora/open", "remora:"] {
            #expect(link(refused) == nil, "\(refused)")
        }
    }
}

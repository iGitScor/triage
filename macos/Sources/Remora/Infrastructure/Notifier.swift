import AppKit
import RemoraCore
import UserNotifications

/// A button on a notification.
enum NotificationAction: String, CaseIterable {
    case done = "remora.done"
    case snoozeTenMinutes = "remora.snooze.10m"
    case snoozeHour = "remora.snooze.1h"
    case snoozeTomorrow = "remora.snooze.tomorrow"
    case start = "remora.start"

    var title: String {
        switch self {
        case .done: L("Done")
        case .snoozeTenMinutes: L("10 more minutes")
        case .snoozeHour: L("Snooze 1 hour")
        case .snoozeTomorrow: L("Tomorrow 9:00")
        case .start: L("Start")
        }
    }

    var symbol: String {
        switch self {
        case .done: "checkmark"
        case .snoozeTenMinutes, .snoozeHour: "moon.zzz"
        case .snoozeTomorrow: "sunrise"
        case .start: "play"
        }
    }
}

/// What the inbox asks of notifications: the Notification Center in the app, a recorder in tests.
@MainActor
protocol Notifying: AnyObject {
    func post(_ notice: Notice)
    func schedule(_ notice: Notice, at date: Date)
    func cancel(_ itemID: String)
    func removeAll()
    func remove(itemsWithPrefix prefix: String)
}

/// Posts and schedules user notifications. Clicking one opens the related link;
/// its buttons are forwarded to `onAction`.
/// On the main actor: its callbacks reach the inbox, which lives there. The system calls its delegate methods
/// from anywhere, so they take plain values out and hop over.
@MainActor
final class Notifier: NSObject, UNUserNotificationCenterDelegate, Notifying {
    static let shared = Notifier()

    var onAction: (@MainActor (NotificationAction, _ itemID: String) -> Void)?
    /// Opens the item the way a click in the inbox would (desktop app first). Returns false if unknown.
    var onOpen: (@MainActor (_ itemID: String) -> Bool)?

    private enum Category: String {
        case item = "remora.item"
        case reminder = "remora.reminder"

        var actions: [NotificationAction] {
            switch self {
            case .item: [.done, .snoozeHour, .snoozeTomorrow]
            case .reminder: [.start, .done, .snoozeTenMinutes, .snoozeHour]
            }
        }
    }

    private var center: UNUserNotificationCenter? {
        Bundle.main.bundleIdentifier == nil ? nil : .current()
    }

    /// True when the user turned Remora's notifications off (or never allowed them) in System Settings.
    func isDenied() async -> Bool {
        guard let center else { return false }
        return await center.notificationSettings().authorizationStatus == .denied
    }

    /// System Settings → Notifications, at Remora when macOS can.
    static func openSystemSettings() {
        let id = Bundle.main.bundleIdentifier ?? ""
        let url = URL(string: "x-apple.systempreferences:com.apple.Notifications-Settings.extension?id=\(id)")
            ?? URL(string: "x-apple.systempreferences:com.apple.Notifications-Settings.extension")!
        NSWorkspace.shared.open(url)
    }

    func activate() {
        center?.delegate = self
        center?.requestAuthorization(options: [.alert, .sound, .badge]) { _, _ in }
        center?.setNotificationCategories(Set([Category.item, .reminder].map { category in
            UNNotificationCategory(
                identifier: category.rawValue,
                actions: category.actions.map { action in
                    UNNotificationAction(
                        identifier: action.rawValue,
                        title: action.title,
                        options: [],
                        icon: UNNotificationActionIcon(systemImageName: action.symbol)
                    )
                },
                intentIdentifiers: []
            )
        }))
    }

    func post(_ notice: Notice) {
        add(id: UUID().uuidString, notice: notice, trigger: nil)
    }

    func schedule(_ notice: Notice, at date: Date) {
        let components = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute, .second], from: date)
        add(id: notice.itemID, notice: notice, trigger: UNCalendarNotificationTrigger(dateMatching: components, repeats: false))
    }

    func cancel(_ itemID: String) {
        center?.removePendingNotificationRequests(withIdentifiers: [itemID])
    }

    /// Scheduled and delivered notifications show titles and messages: Erase removes them all.
    func removeAll() {
        center?.removeAllPendingNotificationRequests()
        center?.removeAllDeliveredNotifications()
    }

    /// Removes the scheduled and delivered notifications of items whose id starts with `prefix` (an account's).
    func remove(itemsWithPrefix prefix: String) {
        guard let center else { return }
        // The handlers run on another thread: they take the shared center there rather than capturing this one.
        center.getPendingNotificationRequests { requests in
            let ids = requests.filter { Self.itemID(of: $0.content).hasPrefix(prefix) || $0.identifier.hasPrefix(prefix) }.map(\.identifier)
            UNUserNotificationCenter.current().removePendingNotificationRequests(withIdentifiers: ids)
        }
        center.getDeliveredNotifications { notifications in
            let ids = notifications.filter { Self.itemID(of: $0.request.content).hasPrefix(prefix) }.map(\.request.identifier)
            UNUserNotificationCenter.current().removeDeliveredNotifications(withIdentifiers: ids)
        }
    }

    nonisolated private static func itemID(of content: UNNotificationContent) -> String {
        content.userInfo["itemID"] as? String ?? content.threadIdentifier
    }

    private func add(id: String, notice: Notice, trigger: UNNotificationTrigger?) {
        let content = UNMutableNotificationContent()
        content.title = notice.title
        content.subtitle = notice.subtitle
        content.body = notice.body
        content.sound = .default
        content.threadIdentifier = notice.itemID
        content.categoryIdentifier = (notice.kind == .reminder ? Category.reminder : .item).rawValue
        content.userInfo["itemID"] = notice.itemID
        if let url = notice.url { content.userInfo["url"] = url.absoluteString }
        center?.add(UNNotificationRequest(identifier: id, content: content, trigger: trigger))
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        [.banner, .sound]
    }

    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        let info = response.notification.request.content.userInfo
        let itemID = info["itemID"] as? String
        let url = (info["url"] as? String).flatMap(URL.init(string:))
        let identifier = response.actionIdentifier
        if let action = NotificationAction(rawValue: identifier), let itemID {
            await MainActor.run { onAction?(action, itemID) }
        } else if identifier == UNNotificationDefaultActionIdentifier {
            await MainActor.run {
                if let itemID, onOpen?(itemID) == true { return }
                if let url, LinkPolicy.isWebLink(url) { NSWorkspace.shared.open(url) }
            }
        }
    }
}

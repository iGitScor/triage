import AppKit
import RemoraCore
import UserNotifications

/// A button on a notification.
enum NotificationAction: String, CaseIterable {
    case done = "remora.done"
    case snoozeTenMinutes = "remora.snooze.10m"
    case snoozeHour = "remora.snooze.1h"
    case snoozeTomorrow = "remora.snooze.tomorrow"

    var title: String {
        switch self {
        case .done: L("Done")
        case .snoozeTenMinutes: L("10 more minutes")
        case .snoozeHour: L("Snooze 1 hour")
        case .snoozeTomorrow: L("Tomorrow 9:00")
        }
    }

    var symbol: String {
        switch self {
        case .done: "checkmark"
        case .snoozeTenMinutes, .snoozeHour: "moon.zzz"
        case .snoozeTomorrow: "sunrise"
        }
    }
}

/// Posts and schedules user notifications. Clicking one opens the related link;
/// its buttons are forwarded to `onAction`.
final class Notifier: NSObject, UNUserNotificationCenterDelegate {
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
            case .reminder: [.done, .snoozeTenMinutes, .snoozeHour]
            }
        }
    }

    private var center: UNUserNotificationCenter? {
        Bundle.main.bundleIdentifier == nil ? nil : .current()
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

    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        [.banner, .sound]
    }

    func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        let info = response.notification.request.content.userInfo
        if let action = NotificationAction(rawValue: response.actionIdentifier), let itemID = info["itemID"] as? String {
            await MainActor.run { onAction?(action, itemID) }
        } else if response.actionIdentifier == UNNotificationDefaultActionIdentifier {
            let itemID = info["itemID"] as? String
            let url = (info["url"] as? String).flatMap(URL.init(string:))
            await MainActor.run {
                if let itemID, onOpen?(itemID) == true { return }
                if let url { NSWorkspace.shared.open(url) }
            }
        }
    }
}

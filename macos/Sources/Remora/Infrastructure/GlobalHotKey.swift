import Carbon.HIToolbox
import Foundation

/// A shortcut that works from any app (MISS-04), through Carbon's hot keys: no accessibility permission needed. One
/// at a time; registering another replaces it.
@MainActor
final class GlobalHotKey {
    private var hotKey: EventHotKeyRef?
    private var handler: EventHandlerRef?
    private let action: () -> Void

    init(action: @escaping () -> Void) {
        self.action = action
    }

    /// False when another app already holds the shortcut: Settings says so.
    @discardableResult
    func register(_ shortcut: Preferences.OpenShortcut) -> Bool {
        unregister()
        guard let (keyCode, modifiers) = shortcut.carbon else { return true }
        if handler == nil {
            var pressed = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
            let me = Unmanaged.passUnretained(self).toOpaque()
            InstallEventHandler(
                GetApplicationEventTarget(),
                { _, _, userData in
                    guard let userData else { return noErr }
                    let hotKey = Unmanaged<GlobalHotKey>.fromOpaque(userData).takeUnretainedValue()
                    MainActor.assumeIsolated { hotKey.action() }
                    return noErr
                }, 1, &pressed, me, &handler)
        }
        let id = EventHotKeyID(signature: OSType(0x524D_5241), id: 1)  // "RMRA"
        let status = RegisterEventHotKey(keyCode, modifiers, id, GetApplicationEventTarget(), 0, &hotKey)
        return status == noErr
    }

    func unregister() {
        if let hotKey { UnregisterEventHotKey(hotKey) }
        hotKey = nil
    }
}

extension Preferences.OpenShortcut {
    /// The key and modifiers, as Carbon names them; nil when off.
    var carbon: (UInt32, UInt32)? {
        switch self {
        case .off: nil
        case .controlOptionR: (UInt32(kVK_ANSI_R), UInt32(controlKey | optionKey))
        case .controlOptionSpace: (UInt32(kVK_Space), UInt32(controlKey | optionKey))
        case .optionCommandI: (UInt32(kVK_ANSI_I), UInt32(optionKey | cmdKey))
        }
    }
}

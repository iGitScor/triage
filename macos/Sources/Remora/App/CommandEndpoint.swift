import Foundation
import RemoraCore

/// The `remora` command (IDEA-22): `remora count`, `list`, `remind` ask the app on its socket (`LocalSocket`). A demo
/// run answers only where REMORA_SOCKET says, never in the real app's place.
@MainActor enum CommandEndpoint {
    static func start(model: InboxModel, demo: Bool) -> LocalSocketServer? {
        let environment = ProcessInfo.processInfo.environment
        if demo && environment[LocalSocket.variable] == nil { return nil }
        do {
            return try LocalSocketServer(path: LocalSocket.path(environment: environment)) { line in
                DispatchQueue.main.sync { MainActor.assumeIsolated { model.answerCommand(line) } }
            }
        } catch {
            print("Remora: the remora command can't reach this app: \(error)")
            return nil
        }
    }
}

extension InboxModel {
    /// The inbox as it is now, without the titles of the tools whose content is hidden (Settings, or the
    /// organization's policy).
    func answerCommand(_ line: String, now: Date = .now) -> String {
        let hidden = effectivePreferences.hiddenContentPlugins
        let remind = { (text: String, at: Date) throws(RemoraCommand.Refusal) -> String in
            guard let id = self.addReminder(text, at: at) else {
                throw RemoraCommand.Refusal("Remora couldn't save it.")
            }
            return id
        }
        return RemoraCommand.answer(line, layout: layout, hides: { hidden.contains($0) }, now: now, remind: remind)
    }
}

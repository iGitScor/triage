import AppKit

/// Remora with sample data in a window of its own: a second instance in `--demo`, which saves nothing, so the app can
/// be seen working before creating a token. One at a time; closed once a real tool is connected. Same as Windows.
@MainActor
enum SampleData {
    private static var running: NSRunningApplication?
    private static var process: Process?

    static func open() {
        if running?.isTerminated == false || process?.isRunning == true { return }
        let bundle = Bundle.main.bundleURL
        if bundle.pathExtension == "app" {
            let configuration = NSWorkspace.OpenConfiguration()
            configuration.arguments = ["--demo"]
            configuration.createsNewApplicationInstance = true
            NSWorkspace.shared.openApplication(at: bundle, configuration: configuration) { app, _ in
                Task { @MainActor in running = app }
            }
        } else if let executable = Bundle.main.executableURL {
            // Run from the build folder (swift run): the executable itself.
            let started = Process()
            started.executableURL = executable
            started.arguments = ["--demo"]
            try? started.run()
            process = started
        }
    }

    /// A real tool is connected: the sample data has done its job.
    static func close() {
        running?.terminate()
        process?.terminate()
        running = nil
        process = nil
    }
}

import Testing
@testable import RemoraPlugins

struct RegistryTests {
    @Test func pluginIDsAreUnique() {
        let ids = PluginRegistry.sources.map { $0.manifest.id }
        #expect(Set(ids).count == ids.count)
    }
}

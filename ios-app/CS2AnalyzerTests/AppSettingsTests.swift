import XCTest
@testable import CS2Analyzer

final class AppSettingsTests: XCTestCase {
    private func makeDefaults() -> UserDefaults {
        let suiteName = "app-settings-test-\(UUID().uuidString)"
        return UserDefaults(suiteName: suiteName)!
    }

    func testDefaultsToOneGigabyte() {
        let settings = AppSettings(defaults: makeDefaults())
        XCTAssertEqual(settings.maxCacheBytes, AppSettings.defaultMaxCacheBytes)
    }

    func testPersistsAcrossInstances() {
        let defaults = makeDefaults()
        let first = AppSettings(defaults: defaults)
        first.maxCacheBytes = 512 * 1_048_576

        let second = AppSettings(defaults: defaults)
        XCTAssertEqual(second.maxCacheBytes, 512 * 1_048_576)
    }
}

import XCTest
@testable import CS2Analyzer

/// Сквозной тест этапа I9: выбор файла -> мост -> `MatchStore` -> список. Как и
/// `LocalParserTests`, пропускается без `samples/test_demo.dem`.
final class MatchLibraryImportTests: XCTestCase {
    private func sampleDemoURL() -> URL? {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("samples/test_demo.dem")
        return FileManager.default.fileExists(atPath: url.path) ? url : nil
    }

    private func makeIsolatedLibrary() -> MatchLibrary {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("match-library-import-test-\(UUID().uuidString)", isDirectory: true)
        return MatchLibrary(store: MatchStore(rootDirectory: tempDir))
    }

    func testImportAddsTheMatchAndSkipsReparsing() async throws {
        guard let demoURL = sampleDemoURL() else {
            throw XCTSkip("samples/test_demo.dem отсутствует (не в git)")
        }
        let library = makeIsolatedLibrary()
        var sawProgress = false

        let summary = try await library.importDemo(at: demoURL) { _ in sawProgress = true }

        XCTAssertTrue(sawProgress)
        XCTAssertEqual(summary.map, "de_mirage")
        XCTAssertEqual(summary.roundsCount, 10)
        XCTAssertTrue(library.items.contains { $0.id == summary.id })
        XCTAssertNotNil(library.match(for: summary.id))

        // Повторный импорт той же демки не должен снова звать мост — только проверить
        // кэш (`MatchStore`, разд. 9 `ipad-doc.md`) и обновить `lastOpenedAt`.
        var sawProgressAgain = false
        let summaryAgain = try await library.importDemo(at: demoURL) { _ in sawProgressAgain = true }

        XCTAssertFalse(sawProgressAgain)
        XCTAssertEqual(summaryAgain.id, summary.id)
    }
}

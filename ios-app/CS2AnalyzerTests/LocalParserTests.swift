import XCTest
@testable import CS2Analyzer

/// Тест через настоящий C-интерфейс `ios-bridge` (этап I8), не напрямую `parser-core` —
/// поэтому пропускается без `samples/test_demo.dem`, как и тесты `parser-core`/`ios-bridge`
/// самих по себе (демки не лежат в git).
final class LocalParserTests: XCTestCase {
    private func sampleDemoPath() -> String? {
        let path = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // CS2AnalyzerTests
            .deletingLastPathComponent() // ios-app
            .deletingLastPathComponent() // корень репозитория
            .appendingPathComponent("samples/test_demo.dem").path
        return FileManager.default.fileExists(atPath: path) ? path : nil
    }

    func testParsesTheSampleDemoAndReportsProgress() throws {
        guard let demoPath = sampleDemoPath() else {
            throw XCTSkip("samples/test_demo.dem отсутствует (не в git)")
        }
        let outURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("local-parser-test-\(UUID().uuidString).json.gz")
        defer { try? FileManager.default.removeItem(at: outURL) }

        var stages: [ParseStage] = []
        try LocalParser.parse(demoPath: demoPath, outPath: outURL.path) { progress in
            stages.append(progress.stage)
        }

        XCTAssertEqual(stages.last, .done)

        let gzipped = try Data(contentsOf: outURL)
        let json = try gzipped.gunzipped()
        let match = try MatchModel.decode(from: json)
        XCTAssertEqual(match.map, "de_mirage")
        XCTAssertEqual(match.rounds.count, 10)
    }

    func testMissingDemoReportsAnErrorWithoutCrashing() {
        let outURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("local-parser-missing-\(UUID().uuidString).json.gz")

        XCTAssertThrowsError(
            try LocalParser.parse(demoPath: "/no/such/demo.dem", outPath: outURL.path, onProgress: { _ in })
        ) { error in
            XCTAssertTrue(error is LocalParserError)
        }
    }
}

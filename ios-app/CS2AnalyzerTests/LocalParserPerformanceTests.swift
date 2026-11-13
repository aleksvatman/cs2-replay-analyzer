import XCTest
@testable import CS2Analyzer

/// Замеры времени и памяти разбора через `ios-bridge` на устройстве (этап I10,
/// `parser-doc.md` разд. 7 п. 10; `ipad-doc.md` разд. 10/14). Числа печатаются в лог теста,
/// а не пишутся в `docs/measurements.md` напрямую — этот файл принадлежит парсеру/серверу
/// (`ipad-doc.md` разд. 2), запись туда только с разрешения владельца.
///
/// Обе демки не лежат в git — тесты пропускаются без них, как и остальные тесты на
/// реальных демках в этом проекте.
final class LocalParserPerformanceTests: XCTestCase {
    /// На симуляторе процесс теста видит диск Mac напрямую, поэтому путь из `samples/`
    /// репозитория просто существует. На реальном устройстве это не так — файл нужно
    /// заранее положить в `Documents` приложения (например, `xcrun devicectl device copy to
    /// --domain-type appDataContainer --domain-identifier com.avatman.cs2analyzer`), тест
    /// проверяет оба места и пропускается, если не нашёл демку ни там, ни там.
    private func demoPath(fileName: String, repoRelative: String) -> String? {
        let hostPath = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // CS2AnalyzerTests
            .deletingLastPathComponent() // ios-app
            .deletingLastPathComponent() // корень репозитория
            .appendingPathComponent(repoRelative).path
        if FileManager.default.fileExists(atPath: hostPath) { return hostPath }

        let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent(fileName).path
        return FileManager.default.fileExists(atPath: documentsPath) ? documentsPath : nil
    }

    func testShortDemoTimeAndMemory() throws {
        guard let demoPath = demoPath(fileName: "test_demo.dem", repoRelative: "samples/test_demo.dem") else {
            throw XCTSkip("test_demo.dem не найдена ни в samples/, ни в Documents приложения")
        }
        try measureParse(demoPath: demoPath, label: "короткая демка (58 МБ, de_mirage, 10 раундов)")
    }

    func testFullMatchTimeAndMemory() throws {
        guard let demoPath = demoPath(
            fileName: "natus-vincere-vs-falcons-m1-ancient.dem",
            repoRelative: "samples/natus-vincere-vs-falcons-m1-ancient.dem"
        ) else {
            throw XCTSkip("полный матч не найден ни в samples/, ни в Documents приложения")
        }
        try measureParse(demoPath: demoPath, label: "полный матч (410 МБ, de_ancient, 24 раунда)")
    }

    private func measureParse(demoPath: String, label: String) throws {
        let outURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("i10-perf-\(UUID().uuidString).json.gz")
        defer { try? FileManager.default.removeItem(at: outURL) }

        let baselineBytes = residentMemoryBytes()
        let tracker = PeakMemoryTracker()
        tracker.start()
        let start = CFAbsoluteTimeGetCurrent()

        try LocalParser.parse(demoPath: demoPath, outPath: outURL.path, onProgress: { _ in })

        let elapsed = CFAbsoluteTimeGetCurrent() - start
        let peakBytes = tracker.stop()

        let outputSize = (try? FileManager.default.attributesOfItem(atPath: outURL.path)[.size] as? Int) ?? nil
        let footprintMB = Double(peakBytes &- baselineBytes) / 1_048_576
        let peakMB = Double(peakBytes) / 1_048_576
        let outputKB = outputSize.map { Double($0) / 1024 }

        print("""
        [I10] \(label): \
        время \(String(format: "%.1f", elapsed)) с, \
        прирост resident \(String(format: "%.0f", footprintMB)) МБ, \
        пиковый resident \(String(format: "%.0f", peakMB)) МБ, \
        результат \(outputKB.map { String(format: "%.0f КБ", $0) } ?? "?")
        """)

        XCTAssertTrue(FileManager.default.fileExists(atPath: outURL.path))
    }
}

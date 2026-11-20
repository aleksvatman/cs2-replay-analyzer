import XCTest
@testable import CS2Analyzer

final class ErrorLogTests: XCTestCase {
    private func makeLog() -> ErrorLog {
        let fileURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("error-log-test-\(UUID().uuidString).json")
        return ErrorLog(fileURL: fileURL)
    }

    func testAppendAndLoadRoundTrips() {
        let log = makeLog()
        log.append("тест ошибки")

        let entries = log.load()
        XCTAssertEqual(entries.count, 1)
        XCTAssertEqual(entries.first?.message, "тест ошибки")
    }

    func testCapsAtFiftyEntriesDroppingOldest() {
        let log = makeLog()
        for i in 0..<55 {
            log.append("ошибка \(i)")
        }

        let entries = log.load()
        XCTAssertEqual(entries.count, 50)
        XCTAssertEqual(entries.first?.message, "ошибка 5")
        XCTAssertEqual(entries.last?.message, "ошибка 54")
    }

    func testClearRemovesAllEntries() {
        let log = makeLog()
        log.append("что-то пошло не так")
        log.clear()

        XCTAssertTrue(log.load().isEmpty)
    }

    func testFormattedForCopyingHasNewestFirst() {
        let log = makeLog()
        log.append("первая")
        log.append("вторая")

        let text = log.formattedForCopying()
        let firstLineRange = text.range(of: "первая")
        let secondLineRange = text.range(of: "вторая")
        XCTAssertNotNil(firstLineRange)
        XCTAssertNotNil(secondLineRange)
        XCTAssertLessThan(secondLineRange!.lowerBound, firstLineRange!.lowerBound)
    }

    func testRedactsFilesystemPaths() {
        let message = "не удалось открыть /Users/someone/Documents/match.dem: нет доступа"
        let redacted = ErrorLog.redactingPaths(message)

        XCTAssertFalse(redacted.contains("/Users/someone"))
        XCTAssertTrue(redacted.contains("<путь скрыт>"))
    }

    func testLeavesOrdinaryTextUntouched() {
        let message = "демка повреждена: неверная сигнатура"
        XCTAssertEqual(ErrorLog.redactingPaths(message), message)
    }
}

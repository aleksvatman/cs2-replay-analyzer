import XCTest
@testable import CS2Analyzer

/// Тесты на подложенные вручную матчи (этап I2). Тест hosted (`TEST_HOST`), поэтому
/// `Bundle.main` внутри него — это бандл приложения `CS2Analyzer`, с тем же
/// `Resources/SampleMatches`, что видит сама `MatchLibrary`.
final class MatchLibraryTests: XCTestCase {
    func testLoadsBundledSampleMatches() {
        let library = MatchLibrary()

        XCTAssertEqual(library.loadErrors, [])
        XCTAssertEqual(library.items.count, 2)

        let mirage = try! XCTUnwrap(library.items.first { $0.id == "sample-mirage" })
        XCTAssertEqual(mirage.map, "de_mirage")
        XCTAssertEqual(mirage.roundsCount, 10)
        XCTAssertEqual(mirage.scoreT, 8)
        XCTAssertEqual(mirage.scoreCT, 2)

        let ancient = try! XCTUnwrap(library.items.first { $0.id == "sample-ancient" })
        XCTAssertEqual(ancient.map, "de_ancient")
        XCTAssertEqual(ancient.roundsCount, 24)
        XCTAssertEqual(ancient.scoreT, 11)
        XCTAssertEqual(ancient.scoreCT, 13)

        XCTAssertNotNil(library.match(for: "sample-mirage"))
        XCTAssertNotNil(library.match(for: "sample-ancient"))
        XCTAssertNil(library.match(for: "no-such-id"))
    }

    func testPlayerNameLookup() throws {
        let library = MatchLibrary()
        let match = try XCTUnwrap(library.match(for: "sample-mirage"))
        let firstPlayer = try XCTUnwrap(match.players.first)

        XCTAssertEqual(match.playerName(firstPlayer.index), firstPlayer.name)
        XCTAssertNil(match.playerName(-1))
    }
}

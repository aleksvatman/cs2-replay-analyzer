import XCTest
@testable import CS2Analyzer

final class MatchModelTests: XCTestCase {
    private func loadMatch(resource: String) throws -> MatchModel {
        let bundle = Bundle(for: Self.self)
        let url = try XCTUnwrap(bundle.url(forResource: resource, withExtension: "json.gz"))
        let gzipped = try Data(contentsOf: url)
        let json = try gzipped.gunzipped()
        return try MatchModel.decode(from: json)
    }

    /// Тест на эталоне парсера (`samples/test_demo.expected.json.gz`): декодирование не
    /// падает и верхнеуровневые инварианты формата (`docs/format.md`) держатся.
    func testDecodesReferenceMatch() throws {
        let match = try loadMatch(resource: "test_demo.expected")

        XCTAssertEqual(match.schemaVersion, 1)
        XCTAssertEqual(match.map, "de_mirage")
        XCTAssertEqual(match.tickRate, 64)
        XCTAssertFalse(match.players.isEmpty)
        XCTAssertFalse(match.weapons.isEmpty)
        XCTAssertFalse(match.rounds.isEmpty)
        XCTAssertEqual(match.stats.perPlayer.count, match.players.count)

        for round in match.rounds {
            XCTAssertEqual(round.teams.count, match.players.count)
            XCTAssertEqual(round.tracks.count, match.players.count)
            XCTAssertEqual(round.summary.count, match.players.count)
            for track in round.tracks {
                XCTAssertEqual(track.x.count, round.ticks.count)
                XCTAssertEqual(track.y.count, round.ticks.count)
                XCTAssertEqual(track.z.count, round.ticks.count)
                XCTAssertEqual(track.yaw.count, round.ticks.count)
                XCTAssertEqual(track.pitch.count, round.ticks.count)
                XCTAssertEqual(track.hp.count, round.ticks.count)
                XCTAssertEqual(track.money.count, round.ticks.count)
            }
            for grenade in round.grenades {
                XCTAssertEqual(grenade.ticks.count, grenade.x.count)
                XCTAssertEqual(grenade.ticks.count, grenade.y.count)
                XCTAssertEqual(grenade.ticks.count, grenade.z.count)
            }
        }
    }

    /// Декодирование бомбы, гранат, событий и статистики на раунде, где всё это есть
    /// (установка бомбы, убийства, граната с областью дыма/огня).
    func testDecodesRoundDetails() throws {
        let match = try loadMatch(resource: "test_demo.expected")
        let plantedRound = try XCTUnwrap(match.rounds.first { $0.bomb.plantTick != nil })

        XCTAssertNotNil(plantedRound.bomb.x)
        XCTAssertNotNil(plantedRound.bomb.planter)

        let roundWithKill = try XCTUnwrap(match.rounds.first { !$0.events.kills.isEmpty })
        let kill = try XCTUnwrap(roundWithKill.events.kills.first)
        XCTAssertGreaterThanOrEqual(kill.tick, roundWithKill.startTick)

        if let areaGrenade = match.rounds.flatMap(\.grenades).first(where: { $0.area != nil }) {
            XCTAssertTrue(areaGrenade.type == .smoke || areaGrenade.type == .fire)
        }
    }

    /// `schemaVersion`, отличный от поддерживаемого, должен быть отклонён, а не тихо принят.
    func testRejectsUnsupportedSchemaVersion() {
        let json = """
        {"schemaVersion":2,"map":"de_dust2","tickRate":64,"players":[],"weapons":[],"rounds":[],"stats":{"perPlayer":[]}}
        """.data(using: .utf8)!

        XCTAssertThrowsError(try MatchModel.decode(from: json)) { error in
            guard case MatchModelError.unsupportedSchemaVersion(let version) = error else {
                XCTFail("неожиданная ошибка: \(error)")
                return
            }
            XCTAssertEqual(version, 2)
        }
    }

    /// Замер времени и памяти декодирования полного матча (этап I1): полный JSON
    /// (`samples/natus-vincere-vs-falcons-m1-ancient.dem`, 410 МБ демка, разобрана заранее
    /// командой `parse` из `CLAUDE.md`) не хранится в `samples/`, только сжатый результат
    /// в ресурсах теста. Числа печатаются в лог теста, а не пишутся в `docs/measurements.md`
    /// (этот файл принадлежит парсеру/серверу).
    func testDecodeFullMatchPerformanceAndMemory() throws {
        let bundle = Bundle(for: Self.self)
        let url = try XCTUnwrap(bundle.url(forResource: "full_match", withExtension: "json.gz"))
        let gzipped = try Data(contentsOf: url)
        let json = try gzipped.gunzipped()

        let memoryBefore = residentMemoryBytes()
        let start = CFAbsoluteTimeGetCurrent()
        let match = try MatchModel.decode(from: json)
        let elapsed = CFAbsoluteTimeGetCurrent() - start
        let memoryAfter = residentMemoryBytes()

        XCTAssertFalse(match.rounds.isEmpty)
        let deltaMB = Double(memoryAfter - memoryBefore) / 1_048_576
        let jsonMB = Double(json.count) / 1_048_576
        let line = "[I1] decode полного матча: JSON \(String(format: "%.1f", jsonMB)) МБ, "
            + "раундов \(match.rounds.count), время \(String(format: "%.2f", elapsed)) с, "
            + "память +\(String(format: "%.1f", deltaMB)) МБ (resident), "
            + "итого resident \(String(format: "%.1f", Double(memoryAfter) / 1_048_576)) МБ"
        print(line)
        // На симуляторе `print` не всегда долетает до лога xcodebuild; дублируем в файл,
        // чтобы забрать число снаружи после прогона (замер ad hoc, не часть CI).
        try? line.write(
            toFile: NSTemporaryDirectory() + "cs2_decode_measurement.txt",
            atomically: true, encoding: .utf8
        )
    }
}

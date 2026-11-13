import XCTest
@testable import CS2Analyzer

final class MatchStoreTests: XCTestCase {
    private func makeStore() -> MatchStore {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("match-store-test-\(UUID().uuidString)", isDirectory: true)
        return MatchStore(rootDirectory: tempDir)
    }

    private func makeMeta(id: String, lastOpenedAt: Date = Date()) -> CachedMatchMeta {
        CachedMatchMeta(
            id: id, map: "de_mirage", scoreT: 8, scoreCT: 2, roundsCount: 10, playersCount: 10,
            schemaVersion: supportedSchemaVersion, parsedAt: Date(), lastOpenedAt: lastOpenedAt, sizeBytes: 123
        )
    }

    func testStoreAndLoadRoundTrips() throws {
        let store = makeStore()
        let gz = try MatchModelTestsHelpers.sampleMirageGzipped()

        try store.store(id: "abc", gzippedJSON: gz, meta: makeMeta(id: "abc"))

        XCTAssertEqual(store.meta(for: "abc")?.map, "de_mirage")
        let loaded = try store.loadMatch(for: "abc")
        XCTAssertEqual(loaded.map, "de_mirage")
    }

    func testMissingMatchReturnsNilNotThrow() {
        let store = makeStore()
        XCTAssertNil(store.meta(for: "no-such-id"))
        XCTAssertThrowsError(try store.loadMatch(for: "no-such-id"))
    }

    func testListMetaOrdersByLastOpenedDescending() throws {
        let store = makeStore()
        let gz = try MatchModelTestsHelpers.sampleMirageGzipped()

        try store.store(id: "old", gzippedJSON: gz, meta: makeMeta(id: "old", lastOpenedAt: Date(timeIntervalSinceNow: -100)))
        try store.store(id: "new", gzippedJSON: gz, meta: makeMeta(id: "new", lastOpenedAt: Date()))

        XCTAssertEqual(store.listMeta().map(\.id), ["new", "old"])
    }

    func testTouchUpdatesLastOpenedAt() throws {
        let store = makeStore()
        let gz = try MatchModelTestsHelpers.sampleMirageGzipped()
        let old = Date(timeIntervalSinceNow: -1000)
        try store.store(id: "abc", gzippedJSON: gz, meta: makeMeta(id: "abc", lastOpenedAt: old))

        store.touch(id: "abc")

        let updated = try XCTUnwrap(store.meta(for: "abc"))
        XCTAssertGreaterThan(updated.lastOpenedAt, old)
    }

    func testDeleteRemovesMatch() throws {
        let store = makeStore()
        let gz = try MatchModelTestsHelpers.sampleMirageGzipped()
        try store.store(id: "abc", gzippedJSON: gz, meta: makeMeta(id: "abc"))

        store.delete(id: "abc")

        XCTAssertNil(store.meta(for: "abc"))
    }
}

/// Утилита, общая для тестов `MatchStore`/`MatchLibrary`: берёт тот же подложенный матч,
/// что видит сама `MatchLibrary`, уже сжатый (`.json.gz`), как результат настоящего разбора.
/// `Bundle.main`, не `Bundle(for:)` — тест hosted (`TEST_HOST`), `Resources/SampleMatches`
/// лежит в бандле приложения, а не в бандле самого теста (см. `MatchLibraryTests`).
enum MatchModelTestsHelpers {
    static func sampleMirageGzipped() throws -> Data {
        let url = try XCTUnwrap(Bundle.main.url(forResource: "sample_mirage", withExtension: "json.gz"))
        return try Data(contentsOf: url)
    }
}

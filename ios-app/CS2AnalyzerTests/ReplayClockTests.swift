import XCTest
@testable import CS2Analyzer

final class ReplayClockTests: XCTestCase {
    func testStartsAtStartTick() {
        let clock = ReplayClock(startTick: 100, endTick: 500, tickRate: 64)
        XCTAssertEqual(clock.tick, 100)
        XCTAssertEqual(clock.snapshotIndex(), 0)
        XCTAssertFalse(clock.isPlaying)
    }

    func testAdvanceDoesNothingWhilePaused() {
        let clock = ReplayClock(startTick: 100, endTick: 500, tickRate: 64)
        clock.advance(by: 1)
        XCTAssertEqual(clock.tick, 100)
    }

    /// Один тик продвижения в секунду при `tickRate=64` и скорости 1x — 64 тика.
    func testAdvanceMovesByTickRateAtNormalSpeed() {
        let clock = ReplayClock(startTick: 100, endTick: 1000, tickRate: 64)
        clock.isPlaying = true
        clock.advance(by: 1)
        XCTAssertEqual(clock.tick, 164)
    }

    func testAdvanceScalesWithSpeed() {
        let clock = ReplayClock(startTick: 0, endTick: 1000, tickRate: 64)
        clock.isPlaying = true
        clock.speed = .double
        clock.advance(by: 1)
        XCTAssertEqual(clock.tick, 128)
    }

    func testAdvanceStopsAtEndTick() {
        let clock = ReplayClock(startTick: 0, endTick: 100, tickRate: 64)
        clock.isPlaying = true
        clock.advance(by: 10)
        XCTAssertEqual(clock.tick, 100)
        XCTAssertFalse(clock.isPlaying)
    }

    func testSnapshotIndexMatchesStepOfFour() {
        let clock = ReplayClock(startTick: 1000, endTick: 2000, tickRate: 64)
        clock.seek(toTick: 1017)
        // (1017 - 1000) / 4 = 4 (целочисленно).
        XCTAssertEqual(clock.snapshotIndex(), 4)
    }

    func testSeekClampsToRoundBounds() {
        let clock = ReplayClock(startTick: 100, endTick: 500, tickRate: 64)
        clock.seek(toTick: 50)
        XCTAssertEqual(clock.tick, 100)
        clock.seek(toTick: 9999)
        XCTAssertEqual(clock.tick, 500)
    }

    func testTogglePlayRestartsFromBeginningAtEnd() {
        let clock = ReplayClock(startTick: 0, endTick: 100, tickRate: 64)
        clock.seek(toTick: 100)
        clock.togglePlay()
        XCTAssertEqual(clock.tick, 0)
        XCTAssertTrue(clock.isPlaying)
    }
}

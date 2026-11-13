import XCTest
@testable import CS2Analyzer

final class RadarProjectionTests: XCTestCase {
    /// `px = (x - posX) / scale`, `py = (posY - y) / scale`; на единичной калибровке
    /// (posX=0, posY=0, scale=1) проекция — это просто перевод из десятых в целые.
    func testIdentityCalibration() {
        let calibration = RadarCalibration(
            map: "test", image: "test.png", posX: 0, posY: 0, scale: 1, imageSize: 1024
        )
        let projection = RadarProjection(calibration: calibration)

        let point = projection.point(x: 1234, y: -500)
        XCTAssertEqual(point.x, 123.4, accuracy: 0.001)
        XCTAssertEqual(point.y, 50.0, accuracy: 0.001)
    }

    func testOffsetAndScale() {
        let calibration = RadarCalibration(
            map: "test", image: "test.png", posX: -3230, posY: 1713, scale: 5, imageSize: 1024
        )
        let projection = RadarProjection(calibration: calibration)

        // x=1000, y=2000 (десятые) -> игровые 100, 200.
        let point = projection.point(x: 1000, y: 2000)
        XCTAssertEqual(point.x, (100 - (-3230)) / 5, accuracy: 0.001)
        XCTAssertEqual(point.y, (1713 - 200) / 5, accuracy: 0.001)
    }

    /// Точка на самой калибровке (posX, posY в игровых единицах, значит *10 в десятых)
    /// проецируется в угол (0, 0) — верхний левый угол картинки радара.
    func testCalibrationOriginMapsToZero() {
        let calibration = RadarCalibration(
            map: "test", image: "test.png", posX: -3230, posY: 1713, scale: 5, imageSize: 1024
        )
        let projection = RadarProjection(calibration: calibration)

        let point = projection.point(x: Int(calibration.posX * 10), y: Int(calibration.posY * 10))
        XCTAssertEqual(point.x, 0, accuracy: 0.001)
        XCTAssertEqual(point.y, 0, accuracy: 0.001)
    }
}

/// Калибровки реальных карт (этап I3): значения из `resource/overviews/<карта>.txt`,
/// см. `Resources/Maps/README.md`. Тест на самих картинках не завязан — их в репозитории нет.
final class RadarCalibrationStoreTests: XCTestCase {
    func testLoadsBundledMirageCalibration() throws {
        let calibration = try XCTUnwrap(RadarCalibrationStore.calibration(for: "de_mirage"))
        XCTAssertEqual(calibration.posX, -3230)
        XCTAssertEqual(calibration.posY, 1713)
        XCTAssertEqual(calibration.scale, 5)
        XCTAssertEqual(calibration.levels, [])
    }

    func testLoadsBundledAncientCalibration() throws {
        let calibration = try XCTUnwrap(RadarCalibrationStore.calibration(for: "de_ancient"))
        XCTAssertEqual(calibration.posX, -2953)
        XCTAssertEqual(calibration.posY, 2164)
        XCTAssertEqual(calibration.scale, 5)
    }

    func testMissingCalibrationReturnsNil() {
        XCTAssertNil(RadarCalibrationStore.calibration(for: "de_no_such_map"))
    }
}

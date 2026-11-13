import XCTest
@testable import CS2Analyzer

final class WeaponNamesTests: XCTestCase {
    func testKnownEventNameMapsToInventoryStyleDisplayName() {
        XCTAssertEqual(WeaponNames.displayName("ak47"), "AK-47")
        XCTAssertEqual(WeaponNames.displayName("m4a1_silencer"), "M4A1-S")
        XCTAssertEqual(WeaponNames.displayName("awp"), "AWP")
    }

    func testUnknownNamePassesThroughUnchanged() {
        XCTAssertEqual(WeaponNames.displayName("some_future_weapon"), "some_future_weapon")
    }

    /// Всё огнестрельное оружие — одна и та же иконка прицела (чтобы в ленте убийств
    /// пистолеты не выделялись «просто кружком» на фоне остального оружия).
    func testFirearmsShareTheSameIcon() {
        XCTAssertEqual(WeaponNames.icon("glock"), .gun)
        XCTAssertEqual(WeaponNames.icon("ak47"), .gun)
        XCTAssertEqual(WeaponNames.icon("awp"), .gun)
    }

    func testNonFirearmIconsAreDistinct() {
        XCTAssertEqual(WeaponNames.icon("hegrenade"), .grenade)
        XCTAssertEqual(WeaponNames.icon("knife"), .knife)
        XCTAssertEqual(WeaponNames.icon("world"), .world)
        XCTAssertEqual(WeaponNames.icon("unknown_weapon"), .unknown)
    }
}

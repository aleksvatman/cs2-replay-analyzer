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

    func testIconsAreAssignedByCategory() {
        XCTAssertEqual(WeaponNames.icon("ak47"), .rifle)
        XCTAssertEqual(WeaponNames.icon("awp"), .sniper)
        XCTAssertEqual(WeaponNames.icon("hegrenade"), .grenade)
        XCTAssertEqual(WeaponNames.icon("knife"), .knife)
        XCTAssertEqual(WeaponNames.icon("world"), .world)
        XCTAssertEqual(WeaponNames.icon("unknown_weapon"), .unknown)
    }
}

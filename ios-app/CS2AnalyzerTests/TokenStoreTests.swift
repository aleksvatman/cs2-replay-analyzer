import XCTest
@testable import CS2Analyzer

final class TokenStoreTests: XCTestCase {
    override func tearDown() {
        TokenStore.clear()
        super.tearDown()
    }

    func testSaveAndLoadRoundTrips() {
        TokenStore.save("super-secret-token")
        XCTAssertEqual(TokenStore.load(), "super-secret-token")
    }

    func testSaveOverwritesPreviousValue() {
        TokenStore.save("first")
        TokenStore.save("second")
        XCTAssertEqual(TokenStore.load(), "second")
    }

    func testEmptyStringClears() {
        TokenStore.save("something")
        TokenStore.save("")
        XCTAssertNil(TokenStore.load())
    }

    func testLoadWithNothingSavedReturnsNil() {
        XCTAssertNil(TokenStore.load())
    }
}

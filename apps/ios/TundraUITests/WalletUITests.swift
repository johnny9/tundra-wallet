import XCTest

final class WalletUITests: XCTestCase {
    @MainActor func testImportRestartAndReceive() throws {
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.buttons["Import descriptor"].waitForExistence(timeout: 15))
        app.buttons["Import descriptor"].tap()
        let editor = app.textViews["publicDescriptor"]
        XCTAssertTrue(editor.waitForExistence(timeout: 10))
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "single-sig", withExtension: "txt"))
        editor.tap()
        editor.typeText(try String(contentsOf: url, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines))
        app.buttons["Review descriptor"].tap()
        XCTAssertTrue(app.buttons["Add wallet"].waitForExistence(timeout: 10))
        app.buttons["Add wallet"].tap()
        // Import remains reviewable in the sheet until explicitly closed.
        app.buttons["Close"].tap()
        XCTAssertTrue(app.buttons["Receive"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.staticTexts["balance"].label, "— BTC")
        app.buttons["Receive"].tap()
        XCTAssertTrue(app.staticTexts["Receive index 0"].waitForExistence(timeout: 10))
        app.buttons["Close"].tap()
        app.terminate()
        app.launch()
        XCTAssertTrue(app.buttons["Receive"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.staticTexts["balance"].label, "— BTC")
        app.buttons["Receive"].tap()
        XCTAssertTrue(app.staticTexts["Receive index 1"].waitForExistence(timeout: 10))
    }
}

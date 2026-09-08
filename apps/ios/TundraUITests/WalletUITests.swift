import XCTest

final class WalletUITests: XCTestCase {
    @MainActor private func enable(_ toggle: XCUIElement) {
        XCTAssertTrue(toggle.waitForExistence(timeout: 10))
        // SwiftUI includes the long label in the switch's accessibility frame.
        // Tap the trailing control, then assert the actual value before proceeding.
        toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.93, dy: 0.5)).tap()
        XCTAssertEqual(toggle.value as? String, "1")
    }

    @MainActor func testImportRestartAndReceive() async throws {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.buttons["Import descriptor"].waitForExistence(timeout: 15))
        app.buttons["Import descriptor"].tap()
        app.buttons["networkPicker"].tap()
        app.buttons["Regtest"].tap()
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
        app.buttons["Close"].tap()
        app.buttons["Sync test network"].tap()
        let endpoint = app.textFields["Esplora API URL"]
        endpoint.tap()
        endpoint.typeText("http://127.0.0.1:3002\n")
        XCTAssertFalse(app.buttons["Start scan"].isEnabled)
        enable(app.switches["syncConsent"])
        XCTAssertTrue(app.buttons["Start scan"].isEnabled)
        app.buttons["Start scan"].tap()
        let funded = NSPredicate(format: "label == %@", "150 BTC")
        let fundedExpectation = expectation(for: funded, evaluatedWith: app.staticTexts["balance"])
        await fulfillment(of: [fundedExpectation], timeout: 30)
        app.buttons["Coins"].tap()
        let selections = app.buttons.matching(identifier: "Select coin")
        XCTAssertEqual(selections.count, 3)
        selections.element(boundBy: 0).tap()
        selections.element(boundBy: 1).tap()
        app.buttons["Consolidate"].tap()
        XCTAssertTrue(app.staticTexts["Fee rate"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["sat/vB"].exists)
        enable(app.switches["consolidationConsent"])
        app.buttons["Review payment"].tap()
        let inputCount = app.staticTexts.matching(NSPredicate(format: "label ==[c] %@", "Inputs · 2")).firstMatch
        XCTAssertTrue(inputCount.waitForExistence(timeout: 10))
        app.buttons["Save for later"].tap()
        app.terminate()
        app.launch()
        XCTAssertTrue(app.buttons["Activity"].waitForExistence(timeout: 10))
        app.buttons["Activity"].tap()
        XCTAssertTrue(app.staticTexts["Draft · unsigned · 2 inputs"].waitForExistence(timeout: 10))
    }
}

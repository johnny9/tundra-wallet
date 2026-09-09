import XCTest

final class SigningUITests: XCTestCase {
    private enum ControlFailure: Error { case unavailable }
    private struct Fixture: Decodable {
        struct Final: Decodable { let txid: String; let vsize: UInt64; let fee_sats: UInt64 }
        let transaction: Final
        enum CodingKeys: String, CodingKey { case transaction = "final" }
    }
    private func fixture() throws -> Fixture {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "native-signing", withExtension: "json"))
        return try JSONDecoder().decode(Fixture.self, from: Data(contentsOf: url))
    }
    private func posts() async throws -> Int {
        var request = URLRequest(url: URL(string: "http://127.0.0.1:3004/_fixture_state")!)
        request.timeoutInterval = 3
        let (data, response) = try await URLSession.shared.data(for: request)
        XCTAssertEqual((response as? HTTPURLResponse)?.statusCode, 200)
        return try XCTUnwrap((JSONSerialization.jsonObject(with: data) as? [String: Int])?["posts"])
    }
    @MainActor private func launch(_ scenario: String) throws -> XCUIApplication {
        continueAfterFailure = false
        executionTimeAllowance = 300
        let app = XCUIApplication()
        app.launchEnvironment = ["TUNDRA_PUBLIC_UI_ID": UUID().uuidString, "TUNDRA_PUBLIC_UI_SCENARIO": scenario]
        app.launch(); try ready(app); return app
    }
    @MainActor private func ready(_ app: XCUIApplication) throws {
        guard app.staticTexts["Public fixture ready"].waitForExistence(timeout: 30),
              !app.staticTexts["Public fixture operation failed"].exists else {
            XCTFail("The public native fixture did not become ready"); throw ControlFailure.unavailable
        }
    }
    @MainActor private func show(_ element: XCUIElement, in app: XCUIApplication) throws {
        for _ in 0..<10 {
            if element.exists && element.isHittable { return }
            app.swipeUp()
        }
        XCTFail("The requested native control was not visible"); throw ControlFailure.unavailable
    }
    @MainActor private func tap(_ element: XCUIElement, in app: XCUIApplication) throws {
        try show(element, in: app); guard element.isEnabled else { throw ControlFailure.unavailable }; element.tap()
    }
    @MainActor private func enable(_ element: XCUIElement, in app: XCUIApplication) throws {
        try show(element, in: app); guard element.isEnabled else { throw ControlFailure.unavailable }
        element.coordinate(withNormalizedOffset: CGVector(dx: 0.93, dy: 0.5)).tap()
        guard element.value as? String == "1" else { throw ControlFailure.unavailable }
    }
    @MainActor private func replace(_ field: XCUIElement, with value: String, in app: XCUIApplication) throws {
        try show(field, in: app)
        guard let current = field.value as? String, current.count <= 256 else { throw ControlFailure.unavailable }
        // Avoid partial-word selection in URLs and trailing-aligned amount fields.
        field.coordinate(withNormalizedOffset: CGVector(dx: 0.99, dy: 0.5)).tap()
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: current.count) + value)
        guard field.value as? String == value else { throw ControlFailure.unavailable }
        app.buttons["Done"].tap()
    }

    @MainActor func testPublishedFileResponseUpdatesSignatureCountersAndFinalizationControl() async throws {
        let expected = try fixture().transaction
        let app = try launch("signing")
        XCTAssertTrue(app.staticTexts["Network: Signet"].exists)
        try show(app.staticTexts["Input 1: 0 / 1"], in: app)
        try show(app.staticTexts["Input 2: 0 / 1"], in: app)
        XCTAssertFalse(app.buttons["Finalize for review"].exists)
        // Only fixture delivery is test-specific. The model calls the same bounded
        // native file reader used by the production signed-PSBT picker callback.
        app.buttons["Supply published response"].tap(); try ready(app)
        try show(app.staticTexts["Input 1: 1 / 1"], in: app)
        try show(app.staticTexts["Input 2: 1 / 1"], in: app)
        try tap(app.buttons["Finalize for review"], in: app); try ready(app)
        try show(app.staticTexts["Final transaction"], in: app)
        try show(app.staticTexts[expected.txid].firstMatch, in: app)
        try show(app.staticTexts["\(expected.vsize.formatted()) vB · \(expected.fee_sats.formatted()) sats fee"], in: app)
        let count = try await posts(); XCTAssertEqual(count, 0)
        app.terminate(); app.launch(); try ready(app)
        try show(app.staticTexts["Final transaction"], in: app)
        try show(app.staticTexts[expected.txid].firstMatch, in: app)
        let afterRestart = try await posts(); XCTAssertEqual(afterRestart, 0)
    }

    @MainActor func testRecoveredApprovalRequiresSyncAndSeparateSubmissionThenShowsInputHistory() async throws {
        let app = try launch("recovery")
        let resume = app.buttons["resumeRecovery"]
        try show(resume, in: app); XCTAssertFalse(resume.isEnabled)
        XCTAssertFalse(app.switches["recoveryConsent"].isEnabled)
        let initial = try await posts(); XCTAssertEqual(initial, 0)
        app.buttons["Sync public fixture"].tap(); try ready(app)
        try show(resume, in: app); XCTAssertFalse(resume.isEnabled)
        try enable(app.switches["recoveryConsent"], in: app)
        try tap(resume, in: app); try ready(app)
        let afterReview = try await posts(); XCTAssertEqual(afterReview, 0)
        try tap(app.buttons["reviewBroadcast"], in: app)
        let submit = app.buttons["confirmBroadcast"]
        try show(submit, in: app); XCTAssertFalse(submit.isEnabled)
        // Return to the endpoint near the top after checking the disabled control.
        app.swipeDown(); app.swipeDown()
        try replace(app.textFields["broadcastEndpoint"], with: "http://127.0.0.1:3004", in: app)
        try enable(app.switches["broadcastConsent"], in: app)
        try show(submit, in: app); XCTAssertFalse(submit.isEnabled)
        try enable(app.switches["broadcastRetryConsent"], in: app)
        try tap(submit, in: app); try ready(app)
        let afterSubmit = try await posts(); XCTAssertEqual(afterSubmit, 1)
        app.terminate(); app.launch(); try ready(app)
        let afterRestart = try await posts(); XCTAssertEqual(afterRestart, 1)
        try show(app.staticTexts["The endpoint acknowledged receipt. The transaction has not been observed by wallet sync."], in: app)
        app.buttons["Show wallet"].tap()
        app.buttons["Sync public fixture"].tap(); try ready(app)
        app.buttons["Coins"].tap()
        let coins = app.buttons.matching(identifier: "coinDetails")
        XCTAssertEqual(coins.count, 1); try tap(coins.firstMatch, in: app)
        XCTAssertTrue(app.staticTexts["Original input labels · historical record"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["Created by Published vector"].exists)
        XCTAssertEqual(app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "Public vector · ")).count, 2)
        try replace(app.textFields["Label"], with: "Public UI edited output", in: app)
        try tap(app.buttons["Save"], in: app); try ready(app)
        app.buttons["Sync public fixture"].tap(); try ready(app)
        XCTAssertTrue(app.staticTexts["Public UI edited output"].exists)
        let afterObservation = try await posts(); XCTAssertEqual(afterObservation, 1)
    }
}

import XCTest

final class WalletUITests: XCTestCase {
    @MainActor private func visibleDocumentItem(_ label: String, in app: XCUIApplication) -> XCUIElement? {
        app.descendants(matching: .any).matching(NSPredicate(format: "label == %@", label))
            .allElementsBoundByIndex.first { $0.isHittable }
    }

    @MainActor private func localBackupFolder(_ app: XCUIApplication) async -> Bool {
        // Only inspect named controls in this disposable public-fixture test. Never
        // log a document/UI tree, user paths or private wallet contents.
        for _ in 0..<8 {
            if let folder = visibleDocumentItem("Backups", in: app) { folder.tap(); return true }
            if let folder = visibleDocumentItem("Tundra", in: app) { folder.tap() }
            else if let local = visibleDocumentItem("On My iPhone", in: app) { local.tap() }
            else if let browse = visibleDocumentItem("Browse", in: app) { browse.tap() }
            else if let locations = visibleDocumentItem("Locations", in: app) { locations.tap() }
            try? await Task.sleep(for: .milliseconds(350))
        }
        XCTFail("The local public-backup document folder was not available"); return false
    }

    @MainActor private func backupDocumentRoundTrip(_ app: XCUIApplication) async -> Bool {
        let password = "Public document test password 2026"
        app.buttons["Settings"].tap(); app.buttons["Backup and recovery"].tap()
        let first = app.secureTextFields["backupPassword"]
        guard first.waitForExistence(timeout: 10) else { XCTFail("Backup password control missing"); return false }
        first.tap(); first.typeText(password)
        let confirmation = app.secureTextFields["backupPasswordConfirmation"]
        confirmation.tap(); confirmation.typeText(password)
        app.buttons["Done"].tap()
        guard await tapVisible(app.buttons["prepareBackup"], in: app) else { return false }
        guard await localBackupFolder(app) else { return false }
        let save = app.buttons.matching(NSPredicate(format: "label IN %@", ["Save", "Export", "Move"]))
            .allElementsBoundByIndex.first { $0.isEnabled && $0.isHittable }
        guard let save else { XCTFail("System backup save control missing"); return false }
        save.tap()
        let complete = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == true AND enabled == true"), object: first)
        guard await XCTWaiter.fulfillment(of: [complete], timeout: 20) == .completed else {
            XCTFail("Backup export/readback did not finish"); return false
        }
        app.swipeUp()
        let result = app.staticTexts["backupResult"]
        guard result.waitForExistence(timeout: 10), result.label.hasPrefix("Encrypted backup saved and read back successfully") else {
            XCTFail("System document export did not produce verified success"); return false
        }
        app.buttons["Close"].tap()
        // A receive issued after the backup is intentionally absent from that snapshot.
        // The recovery screen must explain this limit before the user accepts it.
        app.buttons["Receive"].tap()
        guard app.staticTexts["receiveIndex"].waitForExistence(timeout: 10) else { return false }
        let laterIndex = app.staticTexts["receiveIndex"].label
        let laterAddress = app.staticTexts["receiveAddress"].label
        app.buttons["Close"].tap()
        app.buttons["Settings"].tap(); app.buttons["Backup and recovery"].tap()
        app.buttons["Backup action"].tap(); app.buttons["Restore"].tap()
        guard first.waitForExistence(timeout: 10) else { return false }
        first.tap(); first.typeText(password); app.buttons["Done"].tap()
        guard await tapVisible(app.buttons["Choose backup file"], in: app) else { return false }
        var file = app.descendants(matching: .any).matching(NSPredicate(format: "label BEGINSWITH %@", "tundra-backup"))
            .allElementsBoundByIndex.first { $0.isHittable }
        if file == nil {
            guard await localBackupFolder(app) else { return false }
            file = app.descendants(matching: .any).matching(NSPredicate(format: "label BEGINSWITH %@", "tundra-backup"))
                .allElementsBoundByIndex.first { $0.isHittable }
        }
        guard let file else { XCTFail("Exported public backup was not selectable"); return false }
        file.tap()
        let restore = app.buttons["restoreBackup"]
        for _ in 0..<5 { if restore.exists { break }; app.swipeUp() }
        guard restore.waitForExistence(timeout: 20) else { XCTFail("Backup review did not appear"); return false }
        XCTAssertFalse(restore.isEnabled)
        let limitation = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "An old backup cannot know receive addresses issued later.")).firstMatch
        XCTAssertTrue(limitation.exists)
        enable(app.switches["backupRestoreConsent"])
        guard await tapVisible(restore, in: app) else { return false }
        let restored = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label BEGINSWITH %@", "Backup restored."), object: result)
        guard await XCTWaiter.fulfillment(of: [restored], timeout: 20) == .completed else {
            XCTFail("Reviewed backup did not restore"); return false
        }
        app.buttons["Close"].tap()
        XCTAssertTrue(app.staticTexts["balance"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.staticTexts["balance"].label, "— BTC")
        XCTAssertTrue(app.staticTexts["Draft · invalidated · 2 inputs"].waitForExistence(timeout: 10))
        app.terminate(); app.launch()
        XCTAssertTrue(app.buttons["Receive"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.staticTexts["balance"].label, "— BTC")
        app.buttons["Receive"].tap()
        XCTAssertTrue(app.staticTexts["receiveIndex"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.staticTexts["receiveIndex"].label, laterIndex)
        XCTAssertEqual(app.staticTexts["receiveAddress"].label, laterAddress)
        app.buttons["Close"].tap()
        return true
    }
    @MainActor private func enable(_ toggle: XCUIElement) {
        XCTAssertTrue(toggle.waitForExistence(timeout: 10))
        // SwiftUI includes the long label in the switch's accessibility frame.
        // Tap the trailing control, then assert the actual value before proceeding.
        toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.93, dy: 0.5)).tap()
        XCTAssertEqual(toggle.value as? String, "1")
    }

    @MainActor private func tapVisible(_ element: XCUIElement, in app: XCUIApplication) async -> Bool {
        for _ in 0..<6 { if element.isHittable { break }; app.swipeUp() }
        let ready = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == true AND enabled == true AND hittable == true"), object: element)
        guard await XCTWaiter.fulfillment(of: [ready], timeout: 10) == .completed else {
            XCTFail("The requested payment control was not actionable"); return false
        }
        element.tap(); return true
    }
    @MainActor private func discardAndClose(_ app: XCUIApplication) async -> Bool {
        guard await tapVisible(app.buttons["Discard draft and release inputs"], in: app) else { return false }
        let ready = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == true AND enabled == true"), object: app.buttons["Close"])
        guard await XCTWaiter.fulfillment(of: [ready], timeout: 10) == .completed else {
            XCTFail("Discard did not release the review"); return false
        }
        app.buttons["Close"].tap()
        guard app.staticTexts["balance"].waitForExistence(timeout: 10) else {
            XCTFail("The wallet did not return after discarding the draft"); return false
        }
        return true
    }
    @MainActor private func enterPayment(_ app: XCUIApplication, recipient: String, amount: String?) -> Bool {
        let address = app.textFields["Recipient address"]
        XCTAssertTrue(address.waitForExistence(timeout: 10)); address.tap(); address.typeText(recipient)
        if let amount { let field = app.textFields["Amount in BTC"]; field.tap(); field.typeText(amount) }
        let fee = app.textFields["Fee rate in sat/vB"]
        // A coordinate tap can place the caret before the initial value. Select and
        // replace the whole field through the edit menu, then assert the actual value.
        fee.doubleTap()
        let selectAll = app.descendants(matching: .any).matching(identifier: "Select All").firstMatch
        if selectAll.exists { selectAll.tap() }
        let cut = app.descendants(matching: .any).matching(identifier: "Cut").firstMatch
        guard cut.waitForExistence(timeout: 3) else {
            XCTFail("The fee value could not be selected for replacement"); return false
        }
        cut.tap()
        fee.typeText("2.5")
        guard fee.value as? String == "2.5" else {
            XCTFail("The fee field did not contain the exact requested decimal rate"); return false
        }
        app.buttons["Done"].tap()
        return true
    }

    @MainActor func testImportRestartAndReceive() async throws {
        continueAfterFailure = false
        executionTimeAllowance = 420 // All payment modes plus system document recovery/restart.
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
        XCTAssertTrue(app.buttons["Add wallet"].isEnabled)
        XCTAssertTrue(app.buttons["Add wallet"].isHittable)
        app.buttons["Add wallet"].tap()
        // The review disappears only after the wallet is durably imported.
        let imported = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: app.buttons["Add wallet"])
        guard await XCTWaiter.fulfillment(of: [imported], timeout: 10) == .completed else {
            XCTFail("Wallet import did not finish"); return
        }
        // Import remains reviewable in the sheet until explicitly closed.
        app.buttons["Close"].tap()
        XCTAssertTrue(app.buttons["Receive"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.staticTexts["balance"].label, "— BTC")
        app.buttons["Receive"].tap()
        XCTAssertTrue(app.staticTexts["Receive index 0"].waitForExistence(timeout: 10))
        let recipient = app.staticTexts["receiveAddress"].label
        XCTAssertTrue(recipient.hasPrefix("bcrt1"))
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
        let syncResult = await XCTWaiter.fulfillment(of: [fundedExpectation], timeout: 30)
        guard syncResult == .completed else {
            XCTFail("The explicit test-network scan did not produce its expected balance")
            return // Async XCTest failures otherwise cascade into unrelated payment actions.
        }
        // Exercise automatic Send, exact Max and exact Send before the final saved
        // consolidation. Each discarded draft must release inputs for the next mode.
        app.buttons["Send"].tap()
        XCTAssertTrue(app.switches["automaticInputs"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.switches["automaticInputs"].value as? String, "1")
        guard enterPayment(app, recipient: recipient, amount: "0.001") else { return }
        guard await tapVisible(app.buttons["Review payment"], in: app) else { return }
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label ==[c] %@", "Inputs · 1")).firstMatch.waitForExistence(timeout: 10))
        guard await discardAndClose(app) else { return }
        app.buttons["Coins"].tap()
        for max in [true, false] {
            let coins = app.buttons.matching(identifier: "Select coin")
            XCTAssertEqual(coins.count, 3)
            coins.element(boundBy: 0).tap(); coins.element(boundBy: 1).tap()
            app.buttons["Send"].tap()
            if max { app.buttons["paymentMode"].tap(); app.buttons["Max"].tap() }
            else { XCTAssertEqual(app.switches["automaticInputs"].value as? String, "0") }
            guard enterPayment(app, recipient: recipient, amount: max ? nil : "0.001") else { return }
            guard await tapVisible(app.buttons["Review payment"], in: app) else { return }
            XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label ==[c] %@", "Inputs · 2")).firstMatch.waitForExistence(timeout: 10))
            guard await discardAndClose(app) else { return }
        }
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
        let signatureCount = app.staticTexts["Input 1: 0 / 1"]
        for _ in 0..<3 {
            if signatureCount.exists { break }
            app.swipeUp()
        }
        XCTAssertTrue(signatureCount.waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["Import signed PSBT"].exists)
        app.buttons["Save for later"].tap()
        app.terminate()
        app.launch()
        XCTAssertTrue(app.buttons["Activity"].waitForExistence(timeout: 10))
        app.buttons["Activity"].tap()
        XCTAssertTrue(app.staticTexts["Draft · unsigned · 2 inputs"].waitForExistence(timeout: 10))
        guard await backupDocumentRoundTrip(app) else { return }
    }
}

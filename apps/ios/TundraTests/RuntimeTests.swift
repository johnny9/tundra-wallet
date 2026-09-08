import XCTest
@testable import Tundra

final class RuntimeTests: XCTestCase {
    func testSignedFileBoundsAndBinaryFFI() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let fixture = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "hwi-signed-wpkh", withExtension: "psbt"))
        let base64 = try String(contentsOf: fixture, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
        let binary = try XCTUnwrap(Data(base64Encoded: base64))
        let file = directory.appendingPathComponent("response.psbt")
        try binary.write(to: file)
        let service = CoreService()
        do {
            try await service.importSignedDraft("missing-wallet", draftID: "missing-draft", url: file)
            XCTFail("A public response cannot create a draft or wallet")
        } catch AppError.Operation(_, let detail) {
            // Reaching wallet lookup proves the binary file crossed the real mobile FFI
            // and passed PSBT parsing. No app-private state or signatures can be added.
            XCTAssertEqual(detail, "Wallet, draft or coin was not found")
        }
        try Data(repeating: 0, count: 1_398_107).write(to: file)
        do {
            try await service.importSignedDraft("missing-wallet", draftID: "missing-draft", url: file)
            XCTFail("Oversized files must fail before reaching the wallet core")
        } catch let error as CocoaError {
            XCTAssertEqual(error.code, .fileReadCorruptFile)
        }
    }
    func testNativeReopenAndCancellation() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let path = directory.appendingPathComponent("wallet.sqlite").path
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "single-sig", withExtension: "txt"))
        let descriptor = try String(contentsOf: url, encoding: .utf8)
        var core: Tundra? = try Tundra.open(path: path)
        let wallet = try core!.importWallet(name: "Épargne 🧊", payload: descriptor, network: .signet)
        XCTAssertNil(wallet.totalSats)
        let first = try core!.receiveAddress(walletId: wallet.id)
        core = nil
        core = try Tundra.open(path: path)
        XCTAssertEqual(try core!.wallets()[0].name, "Épargne 🧊")
        let second = try core!.receiveAddress(walletId: wallet.id)
        XCTAssertEqual(second.index, first.index + 1)
        XCTAssertNotEqual(first.address, second.address)
        let operation = try core!.prepareSync(walletId: wallet.id, endpoint: "https://unused.invalid", privacyConsent: true)
        XCTAssertEqual(operation.state, .prepared)
        XCTAssertEqual(try core!.cancelSync(operationId: operation.id).state, .cancelled)
        XCTAssertNil(try core!.wallets()[0].syncedAt)
        XCTAssertThrowsError(try core!.previewImport(payload: "private-untrusted-input", network: .signet))
    }
}

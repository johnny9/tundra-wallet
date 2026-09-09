import XCTest
import Vision
import UIKit
@testable import Tundra

final class RuntimeTests: XCTestCase {
    func testPortableBackupAndNativeExportCrossCommonCryptoProvider() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "backup-v1", withExtension: "tundra"))
        let imported = directory.appendingPathComponent("imported.tundra")
        try FileManager.default.copyItem(at: source, to: imported)
        let password = "Public backup test password 🧊 ' spaces "
        let info = try inspectBackup(path: imported.path, password: password)
        XCTAssertEqual(info.wallets.count, 1)
        XCTAssertEqual(info.wallets.first?.name, "Backup public fixture")
        XCTAssertFalse(try XCTUnwrap(info.wallets.first).synced)
        XCTAssertNil(info.wallets.first?.totalSats)
        let before = try Data(contentsOf: imported)
        XCTAssertThrowsError(try inspectBackup(path: imported.path, password: "incorrect public test password"))
        XCTAssertEqual(try Data(contentsOf: imported), before)
        let fixtureURL = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "single-sig", withExtension: "txt"))
        let fixture = try String(contentsOf: fixtureURL, encoding: .utf8)
        let core = try Tundra.openProtected(path: directory.appendingPathComponent("wallet.sqlite").path, storageKey: Data(repeating: 0x11, count: 32))
        let wallet = try core.importWallet(name: "Mobile backup fixture", payload: fixture, network: .signet)
        let output = directory.appendingPathComponent("exported.tundra")
        XCTAssertEqual(try core.exportBackup(path: output.path, password: password).wallets.first?.id, wallet.id)
        XCTAssertEqual(try inspectBackup(path: output.path, password: password).wallets.first?.id, wallet.id)
    }
    func testPlaintextUpgradeRefusesLiveHandlesAndPreservesMobileState() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = directory.appendingPathComponent("upgrade.sqlite")
        let fixture = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "single-sig", withExtension: "txt"))
        let descriptor = try String(contentsOf: fixture, encoding: .utf8)
        let key = Data(repeating: 0x11, count: 32) // Public storage fixture only.
        XCTAssertEqual(try inspectStorage(path: file.path), .missing)
        var core: Tundra? = try Tundra.open(path: file.path)
        let wallet = try core!.importWallet(name: "Upgrade fixture", payload: descriptor, network: .signet)
        let address = try core!.receiveAddress(walletId: wallet.id)
        try core!.setLabel(walletId: wallet.id, kind: "addr", reference: address.address, label: "Migrated public label 🧊")
        XCTAssertEqual(try inspectStorage(path: file.path), .legacyPlaintext)
        XCTAssertThrowsError(try upgradeStorage(path: file.path, storageKey: key))
        core = nil
        try upgradeStorage(path: file.path, storageKey: key)
        XCTAssertEqual(try inspectStorage(path: file.path), .protectedOrUnknown)
        core = try Tundra.openProtected(path: file.path, storageKey: key)
        XCTAssertEqual(try core!.wallets()[0].id, wallet.id)
        XCTAssertEqual(try core!.receiveAddress(walletId: wallet.id).index, 1)
        XCTAssertTrue(try core!.exportLabels(walletId: wallet.id).contains("Migrated public label 🧊"))
        core = nil
        try upgradeStorage(path: file.path, storageKey: key)
    }
    func testProtectedStorageRejectsWrongKeyAndPreservesWallet() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = directory.appendingPathComponent("protected.sqlite")
        let fixture = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "single-sig", withExtension: "txt"))
        let descriptor = try String(contentsOf: fixture, encoding: .utf8)
        // Public database-encryption fixture only; this does not qualify Keychain storage.
        let key = Data(repeating: 0x11, count: 32)
        var core: Tundra? = try Tundra.openProtected(path: file.path, storageKey: key)
        let wallet = try core!.importWallet(name: "Protected public fixture", payload: descriptor, network: .signet)
        let address = try core!.receiveAddress(walletId: wallet.id)
        try core!.setLabel(walletId: wallet.id, kind: "addr", reference: address.address, label: "Protected public label 🧊")
        core = nil
        let before = try Data(contentsOf: file)
        XCTAssertNil(before.range(of: Data("Protected public fixture".utf8)))
        XCTAssertThrowsError(try Tundra.openProtected(path: file.path, storageKey: Data(repeating: 0x22, count: 32)))
        XCTAssertThrowsError(try Tundra.open(path: file.path))
        XCTAssertEqual(try Data(contentsOf: file), before)
        core = try Tundra.openProtected(path: file.path, storageKey: key)
        XCTAssertEqual(try core!.wallets()[0].id, wallet.id)
        XCTAssertNil(try core!.wallets()[0].totalSats)
        XCTAssertEqual(try core!.receiveAddress(walletId: wallet.id).index, 1)
        XCTAssertTrue(try core!.exportLabels(walletId: wallet.id).contains("Protected public label 🧊"))
    }
    private var usedBarcodeRevisions = Set<Int>()
    private func decodeQr(_ frame: String) throws -> String {
        let scaled = try XCTUnwrap(qrBitmap(try renderQrFrame(frame: frame), scale: 8))
        // This is an independent matrix/FFI oracle, not a camera qualification test.
        // Record the decoder revision: a supported older revision can establish barcode
        // interoperability without claiming the simulator's default detector works.
        for revision in VNDetectBarcodesRequest.supportedRevisions.reversed() {
            let request = VNDetectBarcodesRequest(); request.revision = revision; request.symbologies = [.qr]
            do { try VNImageRequestHandler(cgImage: scaled).perform([request]) }
            catch { continue }
            if let text = request.results?.first?.payloadStringValue {
                if usedBarcodeRevisions.insert(revision).inserted {
                    let note = XCTAttachment(string: "Decoded public fixture with Vision revision \(revision); default is \(VNDetectBarcodesRequest.defaultRevision). Camera and default-detector qualification are separate.")
                    note.name = "Independent barcode decoder revision"; note.lifetime = .keepAlways; add(note)
                }
                return text
            }
        }
        let image = XCTAttachment(image: UIImage(cgImage: scaled))
        image.name = "Public QR fixture pixels"; image.lifetime = .keepAlways; add(image)
        XCTFail("Vision could not decode the public QR fixture; exact pixels attached")
        throw CocoaError(.fileReadCorruptFile)
    }
    func testQrMatrixWithIndependentVisionDecoderAndRealFfi() throws {
        let bundle = Bundle(for: Self.self)
        let urURL = try XCTUnwrap(bundle.url(forResource: "qr-registry-psbt", withExtension: "ur"))
        let ur = try String(contentsOf: urURL, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
        let scanner = QrScanner(purpose: .signedPsbt)
        let decoded = try decodeQr(ur)
        XCTAssertEqual(decoded, ur.uppercased())
        XCTAssertEqual(try scanner.receive(frame: decoded).state, .complete)
        XCTAssertEqual(try scanner.payload().prefix(5), Data([112, 115, 98, 116, 255]))
        let fixtureURL = try XCTUnwrap(bundle.url(forResource: "hwi-signed-wpkh", withExtension: "psbt"))
        let base64 = try String(contentsOf: fixtureURL, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
        let expected = try XCTUnwrap(Data(base64Encoded: base64))
        struct Vector: Decodable { let frames: [String] }
        struct Vectors: Decodable { let vectors: [Vector] }
        let vectorURL = try XCTUnwrap(bundle.url(forResource: "qr-bbqr-vectors", withExtension: "json"))
        let vectors = try JSONDecoder().decode(Vectors.self, from: Data(contentsOf: vectorURL))
        for vector in vectors.vectors {
            let session = QrScanner(purpose: .signedPsbt)
            for frame in vector.frames.reversed() {
                let text = try decodeQr(frame)
                XCTAssertEqual(text, frame)
                let progress = try session.receive(frame: text)
                if progress.state != .complete { XCTAssertEqual(try session.receive(frame: text), progress) }
            }
            XCTAssertEqual(try session.progress().state, .complete)
            XCTAssertEqual(try session.payload(), expected)
            XCTAssertEqual(try session.cancel().state, .cancelled)
            XCTAssertThrowsError(try session.payload())
        }
        let descriptorURL = try XCTUnwrap(bundle.url(forResource: "single-sig", withExtension: "txt"))
        let descriptor = try String(contentsOf: descriptorURL, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
        let text = try decodeQr(descriptor)
        let importing = QrScanner(purpose: .descriptor(network: .signet))
        XCTAssertEqual(try importing.receive(frame: text).state, .complete)
        XCTAssertEqual(try importing.payload(), Data(descriptor.utf8))
        let wrongType = QrScanner(purpose: .signedPsbt)
        XCTAssertThrowsError(try wrongType.receive(frame: text))
        XCTAssertEqual(try wrongType.progress().state, .failed)
    }
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
        XCTAssertNil(try core!.broadcastStatus(walletId: wallet.id, draftId: "missing"))
        XCTAssertThrowsError(try core!.broadcastDraft(request: BroadcastRequest(walletId: wallet.id, draftId: "missing",
            endpoint: "http://127.0.0.1:1", expectedTxid: String(repeating: "00", count: 32),
            previousAttempt: nil, privacyConsent: true, retryAcknowledged: false)))
        XCTAssertNil(try core!.broadcastStatus(walletId: wallet.id, draftId: "missing"))
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

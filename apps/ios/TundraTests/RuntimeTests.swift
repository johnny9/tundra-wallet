import XCTest
import Vision
@testable import Tundra

final class RuntimeTests: XCTestCase {
    private func decodeQr(_ frame: String) throws -> String {
        let bitmap = try XCTUnwrap(qrBitmap(try renderQrFrame(frame: frame)))
        let side = bitmap.width * 8
        let context = try XCTUnwrap(CGContext(data: nil, width: side, height: side, bitsPerComponent: 8,
            bytesPerRow: side, space: CGColorSpaceCreateDeviceGray(), bitmapInfo: 0))
        context.interpolationQuality = .none
        context.draw(bitmap, in: CGRect(x: 0, y: 0, width: side, height: side))
        let scaled = try XCTUnwrap(context.makeImage())
        let request = VNDetectBarcodesRequest(); request.symbologies = [.qr]
        try VNImageRequestHandler(cgImage: scaled).perform([request])
        return try XCTUnwrap(request.results?.first?.payloadStringValue)
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
            XCTAssertEqual(session.progress().state, .complete)
            XCTAssertEqual(try session.payload(), expected)
            XCTAssertEqual(session.cancel().state, .cancelled)
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
        XCTAssertEqual(wrongType.progress().state, .failed)
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

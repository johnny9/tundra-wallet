import XCTest
@testable import Tundra

final class StorageStartupTests: XCTestCase {
    func testNormalAppServiceUsesKeychainProtectedStorage() async throws {
        // The hosted test app owns this disposable simulator container. Preserve any
        // existing wallet; this check never deletes storage or replaces a retained key.
        let service = CoreService()
        _ = try await service.wallets()
        let directory = try FileManager.default.url(for: .applicationSupportDirectory,
            in: .userDomainMask, appropriateFor: nil, create: false).appendingPathComponent("Tundra")
        XCTAssertEqual(try inspectStorage(path: directory.appendingPathComponent("wallet.sqlite").path), .protectedOrUnknown)
    }
}

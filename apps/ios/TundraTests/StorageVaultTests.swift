import XCTest
import Security
@testable import Tundra

/// Real simulator Keychain with disposable public descriptors. No Bitcoin signing keys.
final class StorageVaultTests: XCTestCase {
    private func isolated(_ operation: (URL, String) throws -> Void) throws {
        let id = UUID().uuidString
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("vault-test-" + id)
        let service = "dev.johnny9.tundra.test.storage." + id
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer {
            let generations = (try? FileManager.default.contentsOfDirectory(at: directory.appendingPathComponent("restored-stores"),
                includingPropertiesForKeys: nil)) ?? []
            for generation in generations {
                SecItemDelete(query(service + "." + generation.lastPathComponent) as CFDictionary)
            }
            SecItemDelete(query(service) as CFDictionary) // Only this test's unique item.
            try? FileManager.default.removeItem(at: directory)
        }
        // Diagnose simulator signing/access with a non-secret probe. Production errors
        // remain bounded; test failures report only the Keychain OSStatus, never keys.
        var probe = query(service)
        probe[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        probe[kSecValueData as String] = Data([0])
        let status = SecItemAdd(probe as CFDictionary, nil)
        XCTAssertEqual(status, errSecSuccess, "The test host must have a signed app Keychain access group")
        guard status == errSecSuccess else { throw StorageAccessError.unavailable }
        XCTAssertEqual(SecItemDelete(query(service) as CFDictionary), errSecSuccess)
        try operation(directory, service)
    }
    private func fixture() throws -> String {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "single-sig", withExtension: "txt"))
        return try String(contentsOf: url, encoding: .utf8)
    }
    private func query(_ service: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service,
         kSecAttrAccount as String: "database", kSecAttrSynchronizable as String: false]
    }
    private func record(_ service: String) throws -> Data {
        var request = query(service); request[kSecReturnData as String] = true
        var result: CFTypeRef?
        XCTAssertEqual(SecItemCopyMatching(request as CFDictionary, &result), errSecSuccess)
        return try XCTUnwrap(result as? Data)
    }
    private func replace(_ service: String, _ data: Data) {
        XCTAssertEqual(SecItemUpdate(query(service) as CFDictionary,
            [kSecValueData as String: data] as CFDictionary), errSecSuccess)
    }

    func testKeychainReopenPendingMarkerAndLostOrCorruptKeyNeverResetWallet() throws {
        try isolated { directory, service in
            var core: Tundra? = try StorageVault.open(directory: directory, service: service)
            let wallet = try core!.importWallet(name: "Keychain public fixture", payload: fixture(), network: .signet)
            let address = try core!.receiveAddress(walletId: wallet.id)
            try core!.setLabel(walletId: wallet.id, kind: "addr", reference: address.address, label: "Keychain retained label 🧊")
            core = nil
            var retained = try record(service)
            defer { retained.resetBytes(in: 0..<retained.count) }
            XCTAssertEqual(retained.count, 33)
            // Simulate interruption after DB creation but before marking initialization.
            var pending = retained; pending[32] = 0
            replace(service, pending); pending.resetBytes(in: 0..<pending.count)
            core = try StorageVault.open(directory: directory, service: service)
            XCTAssertEqual(try core!.wallets()[0].id, wallet.id)
            XCTAssertEqual(try core!.receiveAddress(walletId: wallet.id).index, 1)
            XCTAssertTrue(try core!.exportLabels(walletId: wallet.id).contains("Keychain retained label 🧊"))
            core = nil
            // Boolean comparisons deliberately never print plaintext storage key bytes.
            XCTAssertTrue(try record(service) == retained)
            let database = directory.appendingPathComponent("wallet.sqlite")
            let before = try Data(contentsOf: database)
            var wrong = retained; wrong[0] ^= 1
            var badMarker = retained; badMarker[32] = 2
            defer { wrong.resetBytes(in: 0..<wrong.count); badMarker.resetBytes(in: 0..<badMarker.count) }
            for invalid in [wrong, badMarker, Data()] {
                replace(service, invalid)
                XCTAssertThrowsError(try StorageVault.open(directory: directory, service: service))
                XCTAssertTrue(try record(service) == invalid)
                XCTAssertTrue(try Data(contentsOf: database) == before)
            }
            replace(service, retained)
            XCTAssertEqual(SecItemDelete(query(service) as CFDictionary), errSecSuccess)
            XCTAssertThrowsError(try StorageVault.open(directory: directory, service: service))
            var absent: CFTypeRef?
            XCTAssertEqual(SecItemCopyMatching(query(service) as CFDictionary, &absent), errSecItemNotFound)
            XCTAssertTrue(try Data(contentsOf: database) == before)
        }
    }

    func testKeychainUpgradeAndInitializedMarkerRefuseMissingDatabase() throws {
        try isolated { directory, service in
            let database = directory.appendingPathComponent("wallet.sqlite")
            var core: Tundra? = try Tundra.open(path: database.path)
            let wallet = try core!.importWallet(name: "Legacy Keychain fixture", payload: fixture(), network: .signet)
            _ = try core!.receiveAddress(walletId: wallet.id)
            core = nil
            XCTAssertEqual(try inspectStorage(path: database.path), .legacyPlaintext)
            core = try StorageVault.open(directory: directory, service: service)
            XCTAssertEqual(try core!.wallets()[0].id, wallet.id)
            XCTAssertEqual(try core!.receiveAddress(walletId: wallet.id).index, 1)
            core = nil
            XCTAssertEqual(try inspectStorage(path: database.path), .protectedOrUnknown)
            var retained = try record(service)
            defer { retained.resetBytes(in: 0..<retained.count) }
            try FileManager.default.removeItem(at: database) // Isolated test-only data.
            XCTAssertThrowsError(try StorageVault.open(directory: directory, service: service))
            XCTAssertFalse(FileManager.default.fileExists(atPath: database.path))
            XCTAssertTrue(try record(service) == retained)
        }
    }

    func testRestoreSwitchesKeychainGenerationsAndRecoversLostKeysAndSelectors() throws {
        try isolated { directory, service in
            let source = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "backup-v1", withExtension: "tundra"))
            let password = "Public backup test password 🧊 ' spaces "
            let backupBytes = try Data(contentsOf: source)
            var core: Tundra? = try StorageVault.openActive(directory: directory, service: service)
            let oldId = try core!.importWallet(name: "Original generation", payload: fixture(), network: .signet).id
            core = nil
            let oldDatabase = directory.appendingPathComponent("wallet.sqlite")
            var oldBytes = try Data(contentsOf: oldDatabase)
            XCTAssertThrowsError(try StorageVault.restoreActive(source: source, password: "Incorrect public password", directory: directory, service: service))
            XCTAssertTrue(try Data(contentsOf: oldDatabase) == oldBytes)
            core = try StorageVault.openActive(directory: directory, service: service)
            XCTAssertEqual(try core!.wallets()[0].id, oldId)
            core = nil
            // Reopening the old core may rewrite authenticated pages with fresh IVs.
            // Subsequent restores must retain the bytes after this legitimate reopen.
            oldBytes = try Data(contentsOf: oldDatabase)
            core = try StorageVault.restoreActive(source: source, password: password, directory: directory, service: service)
            let wallet = try core!.wallets()[0]
            XCTAssertEqual(wallet.name, "Backup public fixture")
            XCTAssertNil(wallet.totalSats); XCTAssertNil(wallet.syncedAt)
            XCTAssertEqual(try core!.receiveAddress(walletId: wallet.id).index, 1)
            core = nil
            var location = try selectedStorage(root: directory.path)
            XCTAssertNotEqual(location.generation, "default"); XCTAssertTrue(location.requireExisting)
            core = try StorageVault.openActive(directory: directory, service: service)
            XCTAssertEqual(try core!.receiveAddress(walletId: wallet.id).index, 2)
            core = nil
            XCTAssertTrue(try Data(contentsOf: oldDatabase) == oldBytes)
            let broken = URL(fileURLWithPath: location.directory).appendingPathComponent("wallet.sqlite")
            let brokenBytes = try Data(contentsOf: broken)
            XCTAssertEqual(SecItemDelete(query(service + "." + location.generation) as CFDictionary), errSecSuccess)
            XCTAssertThrowsError(try StorageVault.openActive(directory: directory, service: service))
            core = try StorageVault.restoreActive(source: source, password: password, directory: directory, service: service)
            core = nil
            XCTAssertNotEqual(try selectedStorage(root: directory.path).generation, location.generation)
            XCTAssertTrue(try Data(contentsOf: broken) == brokenBytes)
            let selector = directory.appendingPathComponent("active-store.v1")
            try Data("corrupt selector".utf8).write(to: selector)
            XCTAssertThrowsError(try StorageVault.openActive(directory: directory, service: service))
            core = try StorageVault.restoreActive(source: source, password: password, directory: directory, service: service)
            core = nil
            try FileManager.default.removeItem(at: selector)
            XCTAssertThrowsError(try StorageVault.openActive(directory: directory, service: service))
            core = try StorageVault.restoreActive(source: source, password: password, directory: directory, service: service)
            core = nil
            location = try selectedStorage(root: directory.path)
            let selected = URL(fileURLWithPath: location.directory).appendingPathComponent("wallet.sqlite")
            try FileManager.default.removeItem(at: selected)
            XCTAssertThrowsError(try StorageVault.openActive(directory: directory, service: service))
            XCTAssertFalse(FileManager.default.fileExists(atPath: selected.path))
            XCTAssertTrue(try Data(contentsOf: source) == backupBytes)
            XCTAssertTrue(try Data(contentsOf: oldDatabase) == oldBytes)
        }
    }
}

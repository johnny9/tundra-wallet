import Foundation
import Security

enum StorageAccessError: Error {
    case unavailable
    static let message = "Wallet storage is unavailable. Existing files were retained; unlock the device or use recovery."
}

/// Database encryption only. No Bitcoin signing keys are generated or retained here.
enum StorageVault {
    private static let initialization = NSRecursiveLock()
    private struct Record { var key: Data; let initialized: Bool }
    private static var appService: String { (Bundle.main.bundleIdentifier ?? "dev.johnny9.tundra.dev") + ".storage.v1" }

    static func openActive(directory: URL, service: String? = nil) throws -> Tundra {
        initialization.lock(); defer { initialization.unlock() }
        do {
            try protectDirectory(directory)
            let location = try selectedStorage(root: directory.path)
            return try open(directory: URL(fileURLWithPath: location.directory, isDirectory: true),
                service: generationService(service ?? appService, location.generation),
                requireExisting: location.requireExisting)
        } catch { throw StorageAccessError.unavailable }
    }

    /// Finish active operations and release the old service core before calling. If
    /// activation fails, reopen selection before further use; the selector may have won.
    static func restoreActive(source: URL, password: String, directory: URL, service: String? = nil) throws -> Tundra {
        initialization.lock(); defer { initialization.unlock() }
        do {
            try protectDirectory(directory)
            let session = try StoreRestoreSession.begin(root: directory.path, databaseName: "wallet.sqlite")
            let location = try session.location()
            let candidate = URL(fileURLWithPath: location.directory, isDirectory: true)
            try protectDirectory(candidate)
            let candidateService = generationService(service ?? appService, location.generation)
            guard try read(service: candidateService) == nil,
                  try inspectStorage(path: candidate.appendingPathComponent("wallet.sqlite").path) == .missing else {
                throw StorageAccessError.unavailable
            }
            var key = Data(count: 32)
            defer { key.resetBytes(in: 0..<key.count) }
            let status = key.withUnsafeMutableBytes { bytes in
                SecRandomCopyBytes(kSecRandomDefault, bytes.count, bytes.baseAddress!)
            }
            guard status == errSecSuccess else { throw StorageAccessError.unavailable }
            guard try insert(Record(key: key, initialized: false), service: candidateService),
                  var retained = try read(service: candidateService) else {
                throw StorageAccessError.unavailable
            }
            defer { retained.key.resetBytes(in: 0..<retained.key.count) }
            guard retained.key == key else { throw StorageAccessError.unavailable }
            _ = try session.restore(source: source.path, password: password, storageKey: key)
            // Verify the retained key and mark it initialized before selection changes.
            let restored = try open(directory: candidate, service: candidateService)
            try session.activate()
            return restored
        } catch { throw StorageAccessError.unavailable }
    }

    private static func generationService(_ base: String, _ generation: String) -> String {
        generation == "default" ? base : base + "." + generation
    }

    private static func protectDirectory(_ directory: URL) throws {
        var root = directory.resolvingSymlinksInPath()
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true,
                                               attributes: [.protectionKey: FileProtectionType.complete])
        try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete], ofItemAtPath: root.path)
        var values = URLResourceValues(); values.isExcludedFromBackup = true
        try root.setResourceValues(values)
    }

    static func open(directory: URL, service: String? = nil, databaseName: String = "wallet.sqlite",
                     requireExisting: Bool = false) throws -> Tundra {
        initialization.lock()
        defer { initialization.unlock() }
        do {
            try protectDirectory(directory)
            let root = directory.resolvingSymlinksInPath()
            let database = root.appendingPathComponent(databaseName)
            let format = try inspectStorage(path: database.path)
            if requireExisting && format != .protectedOrUnknown { throw StorageAccessError.unavailable }
            let service = service ?? appService
            var record: Record
            if let existing = try read(service: service) { record = existing }
            else {
                guard !requireExisting && format != .protectedOrUnknown else { throw StorageAccessError.unavailable }
                var key = Data(count: 32)
                let status = key.withUnsafeMutableBytes { bytes in
                    SecRandomCopyBytes(kSecRandomDefault, bytes.count, bytes.baseAddress!)
                }
                guard status == errSecSuccess else { key.resetBytes(in: 0..<key.count); throw StorageAccessError.unavailable }
                defer { key.resetBytes(in: 0..<key.count) }
                let pending = Record(key: key, initialized: false)
                let added = try insert(pending, service: service)
                // Another process must never cause replacement with our newly generated key.
                guard let retained = try read(service: service) else { throw StorageAccessError.unavailable }
                if added && retained.key != key { throw StorageAccessError.unavailable }
                record = retained
            }
            defer { record.key.resetBytes(in: 0..<record.key.count) }
            if requireExisting && !record.initialized { throw StorageAccessError.unavailable }
            if record.initialized && format != .protectedOrUnknown { throw StorageAccessError.unavailable }
            if format == .legacyPlaintext { try upgradeStorage(path: database.path, storageKey: record.key) }
            let core = try Tundra.openProtected(path: database.path, storageKey: record.key)
            // Pending and initialized records contain exactly the same storage key.
            if !record.initialized { try markInitialized(record, service: service) }
            return core
        } catch { throw StorageAccessError.unavailable }
    }

    private static func identifier(service: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: service, kSecAttrAccount as String: "database",
         kSecAttrSynchronizable as String: false]
    }

    private static func read(service: String) throws -> Record? {
        var query = identifier(service: service)
        query[kSecReturnData as String] = true
        query[kSecReturnAttributes as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess,
              let attributes = result as? [String: Any],
              attributes[kSecAttrAccessible as String] as? String == kSecAttrAccessibleWhenUnlockedThisDeviceOnly as String,
              var bytes = attributes[kSecValueData as String] as? Data else { throw StorageAccessError.unavailable }
        defer { bytes.resetBytes(in: 0..<bytes.count) }
        guard bytes.count == 33, bytes[32] <= 1 else { throw StorageAccessError.unavailable }
        return Record(key: Data(bytes.prefix(32)), initialized: bytes[32] == 1)
    }

    private static func insert(_ record: Record, service: String) throws -> Bool {
        var bytes = record.key; bytes.append(0)
        defer { bytes.resetBytes(in: 0..<bytes.count) }
        var attributes = identifier(service: service)
        attributes[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        attributes[kSecValueData as String] = bytes
        let status = SecItemAdd(attributes as CFDictionary, nil)
        guard status == errSecSuccess || status == errSecDuplicateItem else { throw StorageAccessError.unavailable }
        return status == errSecSuccess
    }

    private static func markInitialized(_ record: Record, service: String) throws {
        var bytes = record.key; bytes.append(1)
        defer { bytes.resetBytes(in: 0..<bytes.count) }
        let status = SecItemUpdate(identifier(service: service) as CFDictionary,
            [kSecValueData as String: bytes] as CFDictionary)
        guard status == errSecSuccess else { throw StorageAccessError.unavailable }
    }
}

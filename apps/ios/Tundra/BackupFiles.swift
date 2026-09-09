import Foundation
import CryptoKit

enum BackupAccessError: Error {
    case unreadable, unverifiedSave
    var message: String {
        switch self {
        case .unreadable: return "Backup could not be opened. Check the password and file; existing wallet files were retained."
        case .unverifiedSave: return "The saved backup could not be verified. The selected file may be incomplete; keep another verified backup."
        }
    }
}

enum BackupFiles {
    static let maxBytes = 256 * 1024 * 1024
    static func stagingDirectory() throws -> URL {
        var directory = try FileManager.default.url(for: .applicationSupportDirectory,
            in: .userDomainMask, appropriateFor: nil, create: true)
            .appendingPathComponent("Tundra/backup-staging", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true,
            attributes: [.protectionKey: FileProtectionType.complete])
        var values = URLResourceValues(); values.isExcludedFromBackup = true
        try directory.setResourceValues(values)
        return directory
    }
    static func exportPath() throws -> URL {
        // A local Files destination is available without a cloud account. Only
        // explicitly exported documents go here; wallet DBs/keys stay in private support storage.
        let local = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask,
            appropriateFor: nil, create: true).appendingPathComponent("Backups", isDirectory: true)
        try FileManager.default.createDirectory(at: local, withIntermediateDirectories: true)
        return try stagingDirectory().appendingPathComponent("export-" + UUID().uuidString + ".tundra")
    }

    static func stage(_ source: URL) throws -> URL {
        let destination = try stagingDirectory().appendingPathComponent("import-" + UUID().uuidString + ".tundra")
        do {
            guard FileManager.default.createFile(atPath: destination.path, contents: nil,
                attributes: [.protectionKey: FileProtectionType.complete]) else { throw BackupAccessError.unreadable }
            let scoped = source.startAccessingSecurityScopedResource()
            defer { if scoped { source.stopAccessingSecurityScopedResource() } }
            try coordinatedRead(source) { coordinated in
                let input = try FileHandle(forReadingFrom: coordinated)
                defer { try? input.close() }
                let output = try FileHandle(forWritingTo: destination)
                defer { try? output.close() }
                _ = try copy(input, output)
                try output.synchronize()
            }
            return destination
        } catch {
            try? FileManager.default.removeItem(at: destination)
            throw BackupAccessError.unreadable
        }
    }

    static func verifySaved(_ destination: URL, source: URL) throws {
        do {
            let scoped = destination.startAccessingSecurityScopedResource()
            defer { if scoped { destination.stopAccessingSecurityScopedResource() } }
            let expected = try digest(source)
            let actual = try coordinatedRead(destination) { try digest($0) }
            guard expected.count >= 4096, expected.count == actual.count, expected.hash == actual.hash else {
                throw BackupAccessError.unverifiedSave
            }
        } catch { throw BackupAccessError.unverifiedSave }
    }

    static func copy(_ input: FileHandle, _ output: FileHandle, max: Int = maxBytes) throws -> Int {
        guard max >= 0 else { throw BackupAccessError.unreadable }
        var total = 0
        while true {
            let chunk = try input.read(upToCount: min(64 * 1024, max - total + 1)) ?? Data()
            if chunk.isEmpty { return total }
            guard chunk.count <= max - total else { throw BackupAccessError.unreadable }
            try output.write(contentsOf: chunk); total += chunk.count
        }
    }

    private static func digest(_ file: URL) throws -> (count: Int, hash: Data) {
        let input = try FileHandle(forReadingFrom: file)
        defer { try? input.close() }
        var count = 0
        var hash = SHA256()
        while true {
            let chunk = try input.read(upToCount: min(64 * 1024, maxBytes - count + 1)) ?? Data()
            if chunk.isEmpty { return (count, Data(hash.finalize())) }
            guard chunk.count <= maxBytes - count else { throw BackupAccessError.unreadable }
            count += chunk.count; hash.update(data: chunk)
        }
    }

    private static func coordinatedRead<T>(_ url: URL, body: (URL) throws -> T) throws -> T {
        var coordinationError: NSError?
        var result: Result<T, Error>?
        NSFileCoordinator().coordinate(readingItemAt: url, options: [], error: &coordinationError) { coordinated in
            result = Result { try body(coordinated) }
        }
        guard coordinationError == nil, let result else { throw BackupAccessError.unreadable }
        return try result.get()
    }
}

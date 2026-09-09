import XCTest
@testable import Tundra

final class BackupFilesTests: XCTestCase {
    func testStagedCiphertextReadbackAndMutationOrTruncationRefusal() throws {
        let source = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "backup-v1", withExtension: "tundra"))
        let staged = try BackupFiles.stage(source)
        defer { try? FileManager.default.removeItem(at: staged) }
        try BackupFiles.verifySaved(staged, source: source)
        var bytes = try Data(contentsOf: staged)
        bytes[100] ^= 1
        try bytes.write(to: staged)
        XCTAssertThrowsError(try BackupFiles.verifySaved(staged, source: source))
        bytes.removeLast()
        try bytes.write(to: staged)
        XCTAssertThrowsError(try BackupFiles.verifySaved(staged, source: source))
    }

    func testBoundedCopyRejectsOverflowAndClosedOutput() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let source = root.appendingPathComponent("source")
        let target = root.appendingPathComponent("target")
        let bytes = Data(repeating: 0x33, count: 513)
        try bytes.write(to: source); try Data().write(to: target)
        let input = try FileHandle(forReadingFrom: source)
        let output = try FileHandle(forWritingTo: target)
        defer { try? input.close(); try? output.close() }
        XCTAssertEqual(try BackupFiles.copy(input, output, max: 513), 513)
        try output.synchronize()
        XCTAssertEqual(try Data(contentsOf: target), bytes)
        try input.seek(toOffset: 0)
        XCTAssertThrowsError(try BackupFiles.copy(input, output, max: 512))
        try input.seek(toOffset: 0); try output.close()
        XCTAssertThrowsError(try BackupFiles.copy(input, output, max: 513))
    }
}

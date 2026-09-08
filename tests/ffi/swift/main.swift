import Foundation

// Compile together with the generated Swift binding and call the real host Rust library.
// This does not exercise SwiftUI or an iOS device's storage/lifecycle behavior.

func importFixture(_ path: String, _ payload: String) throws -> (String, AddressInfo, String) {
    let core = try Tundra.open(path: path)
    let wallet = try core.importWallet(name: "Épargne 🧊", payload: payload, network: .signet)
    precondition(!wallet.synced && wallet.totalSats == nil && wallet.availableSats == nil)
    let address = try core.receiveAddress(walletId: wallet.id)
    precondition(!address.hardwareVerified)
    try core.setLabel(walletId: wallet.id, kind: "addr", reference: address.address, label: "家族の貯蓄 🧊")
    return (wallet.id, address, try core.exportLabels(walletId: wallet.id))
}

func testReopen(_ payload: String) throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let path = directory.appendingPathComponent("wallet.sqlite").path
    let (id, first, labels) = try importFixture(path, payload)
    let core = try Tundra.open(path: path)
    let wallets = try core.wallets()
    precondition(wallets.count == 1 && wallets[0].id == id && wallets[0].name == "Épargne 🧊")
    precondition(!wallets[0].synced && wallets[0].totalSats == nil && wallets[0].availableSats == nil)
    let coins = try core.coins(walletId: id)
    let activity = try core.activity(walletId: id)
    let reopenedLabels = try core.exportLabels(walletId: id)
    precondition(coins.isEmpty && activity.isEmpty && reopenedLabels == labels)
    let next = try core.receiveAddress(walletId: id)
    precondition(next.index == first.index + 1 && next.address != first.address)
}

func testErrors() throws {
    let core = try Tundra.open(path: ":memory:")
    let sentinel = "private-input-must-not-appear-in-errors"
    do {
        _ = try core.previewImport(payload: sentinel, network: .signet)
        preconditionFailure("Invalid input succeeded")
    } catch AppError.Operation(let code, let detail) {
        precondition(code == .invalidInput && !detail.contains(sentinel) && !detail.isEmpty)
    }
}

func testAmounts() throws {
    let sats = try parseBtcAmount(value: "42.94967297")
    precondition(sats == 4_294_967_297)
    precondition(formatBalance(sats: UInt64.max) == "184467440737.09551615")
    do {
        _ = try parseBtcAmount(value: "0.000000001")
        preconditionFailure("Sub-satoshi input succeeded")
    } catch AppError.Operation(let code, _) {
        precondition(code == .invalidInput)
    }
}

precondition(CommandLine.arguments.count == 2, "Pass the public single-sig fixture path")
let fixture = try String(contentsOfFile: CommandLine.arguments[1], encoding: .utf8)
try testReopen(fixture)
try testErrors()
try testAmounts()
print("3 Swift FFI smoke tests passed")

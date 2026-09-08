import Foundation
import SwiftUI

@MainActor
final class WalletModel: ObservableObject {
    @Published var wallets: [WalletInfo] = []
    @Published var selectedID: String?
    @Published var coins: [CoinInfo] = []
    @Published var activity: [ActivityInfo] = []
    @Published var preview: WalletPreview?
    @Published var received: AddressInfo?
    @Published var busy = false
    @Published var error: String?
    private let service = CoreService()
    // Sensitive in-memory edit state is not stored in scene restoration or preferences.
    private var pendingPayload: String?
    private var pendingChain: Chain = .signet
    var wallet: WalletInfo? { wallets.first { $0.id == selectedID } }

    func run(_ operation: @escaping () async throws -> Void) {
        guard !busy else { return }
        busy = true
        error = nil
        Task {
            defer { busy = false }
            do { try await operation() }
            catch is CancellationError { }
            catch { self.error = "The operation could not be completed. Check the test descriptor and selected network." }
        }
    }
    func load() { run { try await self.refresh() } }
    private func refresh() async throws {
        wallets = try await service.wallets()
        if !wallets.contains(where: { $0.id == selectedID }) { selectedID = wallets.first?.id }
        if let id = selectedID {
            coins = try await service.coins(id)
            activity = try await service.activity(id)
        } else { coins = []; activity = [] }
    }
    func select(_ id: String) { run { self.selectedID = id; self.received = nil; try await self.refresh() } }
    func inspect(_ payload: String, chain: Chain) {
        run {
            self.preview = nil; self.pendingPayload = nil
            let preview = try await self.service.inspect(payload, chain: chain)
            self.pendingPayload = payload; self.pendingChain = chain; self.preview = preview
        }
    }
    func inspectFile(_ url: URL, chain: Chain) {
        run {
            self.preview = nil; self.pendingPayload = nil
            let payload = try await self.service.readDescriptor(url)
            let preview = try await self.service.inspect(payload, chain: chain)
            self.pendingPayload = payload; self.pendingChain = chain; self.preview = preview
        }
    }
    func add(_ name: String) {
        run {
            guard let payload = self.pendingPayload else { return }
            let wallet = try await self.service.add(name: name, payload: payload, chain: self.pendingChain)
            self.selectedID = wallet.id; self.cancelImport(); try await self.refresh()
        }
    }
    func cancelImport() { pendingPayload = nil; preview = nil }
    func receive() {
        run {
            guard let id = self.selectedID else { return }
            self.received = try await self.service.receive(id)
        }
    }
}

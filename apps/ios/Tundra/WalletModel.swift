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
    @Published var storageReady = false
    @Published var error: String?
    @Published var endpoint = ""
    @Published var sync: SyncInfo?
    @Published var selectedCoins: Set<String> = []
    @Published var drafts: [PaymentReview] = []
    @Published var review: PaymentReview?
    @Published var outputSource: PaymentReview?
    @Published var sourceOutpoint: String?
    @Published var signing: SigningInfo?
    @Published var finalized: FinalTransactionInfo?
    @Published var submission: BroadcastInfo?
    @Published var recoveryRequired = false
    @Published var qrFrames: [String]?
    @Published var exportedPSBT: String?
    @Published var labelPreview: LabelImportPreview?
    @Published var exportedLabels: String?
    private var pendingLabels: String?
    private var labelsWalletID: String?
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
            catch is StorageAccessError { self.error = StorageAccessError.message }
            catch let error as AppError {
                switch error { case .Operation(_, let detail): self.error = detail }
            }
            catch { self.error = "The operation could not be completed. Check the test descriptor and selected network." }
        }
    }
    func load() { run { try await self.refresh(); self.storageReady = true } }
    private func refresh() async throws {
        signing = nil; finalized = nil; submission = nil; recoveryRequired = false
        wallets = try await service.wallets()
        if !wallets.contains(where: { $0.id == selectedID }) { selectedID = wallets.first?.id }
        if let id = selectedID {
            coins = try await service.coins(id)
            activity = try await service.activity(id)
            endpoint = try await service.endpoint(id)
            drafts = try await service.drafts(id)
            selectedCoins.formIntersection(coins.filter { $0.state == .available }.map(\.outpoint))
            if let old = review { review = drafts.first { $0.id == old.id } }
            if let review {
                submission = try await service.broadcastStatus(review.walletId, draftID: review.id)
                recoveryRequired = try await service.recoveryRequired(review.walletId, draftID: review.id)
            }
            if let review, !["invalidated", "observed"].contains(review.state), submission == nil {
                let progress = try await service.signingProgress(review.walletId, draftID: review.id)
                let finalized = try await service.finalized(review.walletId, draftID: review.id)
                if self.review?.id == review.id { signing = progress; self.finalized = finalized }
            }
        } else { coins = []; activity = [] }
    }
    func select(_ id: String) { run { self.selectedID = id; self.received = nil; self.review = nil; self.qrFrames = nil; self.selectedCoins = []; try await self.refresh() } }
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
        guard let payload = pendingPayload else { return }
        let chain = pendingChain
        run {
            let wallet = try await self.service.add(name: name, payload: payload, chain: chain)
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
    func synchronize(_ endpoint: String, consent: Bool) {
        run {
            guard let id = self.selectedID else { return }
            let operation = try await self.service.prepareSync(id, endpoint: endpoint, consent: consent)
            self.sync = operation
            do {
                try await self.service.runSync(operation.id)
                while true {
                    let progress = try await self.service.progress(operation.id)
                    self.sync = progress
                    if [.complete, .cancelled, .failed].contains(progress.state) {
                        self.error = progress.error
                        try await self.refresh()
                        break
                    }
                    try await Task.sleep(for: .milliseconds(150))
                }
            } catch {
                try? await self.service.cancelSync(operation.id)
                throw error
            }
        }
    }
    func cancelSync() {
        guard let id = sync?.id else { return }
        Task { try? await service.cancelSync(id) }
    }
    func toggleCoin(_ coin: CoinInfo) {
        guard !busy, coin.state == .available else { return }
        if selectedCoins.contains(coin.outpoint) { selectedCoins.remove(coin.outpoint) }
        else { selectedCoins.insert(coin.outpoint) }
    }
    func loadOutputSource(_ outpoint: String) {
        run {
            guard let id = self.selectedID else { return }
            self.outputSource = nil; self.sourceOutpoint = outpoint
            let source = try await self.service.outputSource(id, outpoint: outpoint)
            if self.selectedID == id && self.sourceOutpoint == outpoint { self.outputSource = source }
        }
    }
    func editCoins(_ outpoints: [String], label: String?, frozen: Bool?) {
        run {
            guard let id = self.selectedID else { return }
            try await self.service.editCoins(id, outpoints: outpoints, label: label, frozen: frozen)
            try await self.refresh()
        }
    }
    func createPayment(mode: Int, address: String, amount: String, fee: String, label: String, automatic: Bool, acknowledge: Bool) {
        run {
            guard let id = self.selectedID else { return }
            let intent: PaymentIntent
            switch mode {
            case 1: intent = .sendMax(address: address)
            case 2: intent = .consolidate(privacyAcknowledged: acknowledge)
            default: intent = .send(address: address, sats: try parseBtcAmount(value: amount))
            }
            self.review = try await self.service.create(PaymentRequest(walletId: id, intent: intent,
                selectedOutpoints: automatic && mode == 0 ? nil : Array(self.selectedCoins).sorted(),
                feeSatPerKwu: try parseFeeRate(value: fee), label: label))
            self.selectedCoins = []
            try await self.refresh()
        }
    }
    func discardReview() {
        run {
            guard let review = self.review else { return }
            try await self.service.discard(review.walletId, draftID: review.id)
            self.review = nil; try await self.refresh()
        }
    }
    func exportDraft() {
        run {
            guard let review = self.review else { return }
            self.exportedPSBT = try await self.service.exportDraft(review.walletId, draftID: review.id)
        }
    }
    func openReview(_ review: PaymentReview) {
        self.review = review; signing = nil; finalized = nil; submission = nil; recoveryRequired = false
        run { try await self.refresh() }
    }
    func importSignedDraft(_ url: URL, walletID: String, draftID: String) {
        run {
            guard self.selectedID == walletID, self.review?.id == draftID else { return }
            try await self.service.importSignedDraft(walletID, draftID: draftID, url: url)
            try await self.refresh()
        }
    }
    func importSignedQr(_ payload: Data, walletID: String, draftID: String) {
        run {
            guard self.selectedID == walletID, self.review?.id == draftID else { return }
            try await self.service.importSignedQr(walletID, draftID: draftID, payload: payload)
            try await self.refresh()
        }
    }
    func exportQr(_ encoding: QrEncoding) {
        run {
            guard let review = self.review else { return }
            self.qrFrames = try await self.service.exportQr(review.walletId, draftID: review.id, encoding: encoding)
        }
    }
    func finalizeReview() {
        run {
            guard let review = self.review else { return }
            try await self.service.finalize(review.walletId, draftID: review.id)
            try await self.refresh()
        }
    }
    func submitBroadcast(_ request: BroadcastRequest) {
        run {
            guard self.selectedID == request.walletId, self.review?.id == request.draftId else { return }
            do { try await self.service.broadcast(request) }
            catch { try await self.refresh(); throw error }
            try await self.refresh()
        }
    }
    func resumeRecovery(_ request: RecoveryReviewRequest) {
        run {
            guard self.selectedID == request.walletId, self.review?.id == request.draftId else { return }
            do { try await self.service.resumeRecovery(request) }
            catch { try await self.refresh(); throw error }
            try await self.refresh()
        }
    }
    func closeReview() { review = nil; signing = nil; finalized = nil; submission = nil; recoveryRequired = false; qrFrames = nil }
    func inspectLabels(_ url: URL) {
        run {
            guard let id = self.selectedID else { return }
            let payload = try await self.service.readLabels(url)
            self.labelPreview = try await self.service.importLabels(id, payload: payload, apply: false)
            self.pendingLabels = payload; self.labelsWalletID = id
        }
    }
    func applyLabels() {
        run {
            guard let id = self.labelsWalletID, id == self.selectedID, let payload = self.pendingLabels else { return }
            _ = try await self.service.importLabels(id, payload: payload, apply: true)
            self.cancelLabels(); try await self.refresh()
        }
    }
    func cancelLabels() { pendingLabels = nil; labelsWalletID = nil; labelPreview = nil }
    func exportLabels() {
        run {
            guard let id = self.selectedID else { return }
            self.exportedLabels = try await self.service.exportLabels(id)
        }
    }
}

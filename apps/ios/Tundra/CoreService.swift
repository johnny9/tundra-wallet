import Foundation

/// Database and wallet work never run on the main UI actor.
actor CoreService {
    private var core: Tundra?
    private var pendingBackup: URL?
    private var exportedBackup: URL?

    private func storageDirectory() throws -> URL {
        try FileManager.default.url(for: .applicationSupportDirectory,
            in: .userDomainMask, appropriateFor: nil, create: true)
            .appendingPathComponent("Tundra", isDirectory: true)
    }

    private func engine() throws -> Tundra {
        if let core { return core }
        let directory = try storageDirectory()
        let result = try StorageVault.openActive(directory: directory)
        core = result
        return result
    }
    func wallets() throws -> [WalletInfo] { try engine().wallets() }
    func coins(_ id: String) throws -> [CoinInfo] { try engine().coins(walletId: id) }
    func outputSource(_ id: String, outpoint: String) throws -> PaymentReview? {
        try engine().outputSource(walletId: id, outpoint: outpoint)
    }
    func activity(_ id: String) throws -> [ActivityInfo] { try engine().activity(walletId: id) }
    func inspect(_ payload: String, chain: Chain) throws -> WalletPreview {
        try engine().previewImport(payload: payload, network: chain)
    }
    func add(name: String, payload: String, chain: Chain) throws -> WalletInfo {
        try engine().importWallet(name: name, payload: payload, network: chain)
    }
    func receive(_ id: String) throws -> AddressInfo { try engine().receiveAddress(walletId: id) }
    func drafts(_ id: String) throws -> [PaymentReview] { try engine().drafts(walletId: id) }
    func create(_ request: PaymentRequest) throws -> PaymentReview { try engine().createDraft(request: request) }
    func discard(_ walletID: String, draftID: String) throws { try engine().discardDraft(walletId: walletID, draftId: draftID) }
    func exportDraft(_ walletID: String, draftID: String) throws -> String { try engine().exportSigningPsbt(walletId: walletID, draftId: draftID) }
    func signingProgress(_ walletID: String, draftID: String) throws -> SigningInfo { try engine().signingProgress(walletId: walletID, draftId: draftID) }
    func importSignedDraft(_ walletID: String, draftID: String, url: URL) throws {
        let payload = try readBytes(url, max: 1_398_106)
        _ = try engine().acceptSignedPsbt(walletId: walletID, draftId: draftID, payload: payload)
    }
    func finalize(_ walletID: String, draftID: String) throws { _ = try engine().finalizeDraft(walletId: walletID, draftId: draftID) }
    func finalized(_ walletID: String, draftID: String) throws -> FinalTransactionInfo? { try engine().finalizedDraft(walletId: walletID, draftId: draftID) }
    func broadcastStatus(_ walletID: String, draftID: String) throws -> BroadcastInfo? { try engine().broadcastStatus(walletId: walletID, draftId: draftID) }
    func broadcast(_ request: BroadcastRequest) throws { _ = try engine().broadcastDraft(request: request) }
    func recoveryRequired(_ walletID: String, draftID: String) throws -> Bool {
        try engine().recoveryRequired(walletId: walletID, draftId: draftID)
    }
    func resumeRecovery(_ request: RecoveryReviewRequest) throws { _ = try engine().resumeRecoveredSubmission(request: request) }
    func importSignedQr(_ walletID: String, draftID: String, payload: Data) throws {
        _ = try engine().acceptSignedPsbt(walletId: walletID, draftId: draftID, payload: payload)
    }
    func exportQr(_ walletID: String, draftID: String, encoding: QrEncoding) throws -> [String] {
        try engine().exportDraftQr(walletId: walletID, draftId: draftID, encoding: encoding)
    }
    func editCoins(_ id: String, outpoints: [String], label: String?, frozen: Bool?) throws {
        try engine().editCoins(walletId: id, outpoints: outpoints, label: label, frozen: frozen)
    }
    func endpoint(_ id: String) throws -> String { try engine().syncEndpoint(walletId: id) ?? "" }
    func prepareSync(_ id: String, endpoint: String, consent: Bool) throws -> SyncInfo {
        try engine().prepareSync(walletId: id, endpoint: endpoint, privacyConsent: consent)
    }
    func runSync(_ id: UInt64) throws { try engine().runSync(operationId: id) }
    func progress(_ id: UInt64) throws -> SyncInfo { try engine().syncProgress(operationId: id) }
    func cancelSync(_ id: UInt64) throws { _ = try engine().cancelSync(operationId: id) }
    func importLabels(_ id: String, payload: String, apply: Bool) throws -> LabelImportPreview {
        try engine().importLabels(walletId: id, payload: payload, apply: apply)
    }
    func exportLabels(_ id: String) throws -> String { try engine().exportLabels(walletId: id) }
    func inspectBackupFile(_ source: URL, password: String) throws -> BackupInfo {
        clearBackupFiles()
        let staged = try BackupFiles.stage(source)
        do {
            let info = try inspectBackup(path: staged.path, password: password)
            pendingBackup = staged
            return info
        } catch {
            try? FileManager.default.removeItem(at: staged)
            throw BackupAccessError.unreadable
        }
    }
    func prepareBackup(password: String) throws -> URL {
        clearBackupFiles()
        let file = try BackupFiles.exportPath()
        do {
            _ = try engine().exportBackup(path: file.path, password: password)
            exportedBackup = file
            return file
        } catch { try? FileManager.default.removeItem(at: file); throw error }
    }
    func finishBackupExport(_ destination: URL?) throws {
        guard let file = exportedBackup else { throw BackupAccessError.unverifiedSave }
        defer { try? FileManager.default.removeItem(at: file); exportedBackup = nil }
        if let destination { try BackupFiles.verifySaved(destination, source: file) }
    }
    func restoreBackup(password: String) throws {
        guard let file = pendingBackup else { throw BackupAccessError.unreadable }
        defer { try? FileManager.default.removeItem(at: file); pendingBackup = nil }
        // WalletModel serializes operations and finishes network work before switching.
        // No old handle is retained if activation reports an ambiguous filesystem error.
        core = nil
        core = try StorageVault.restoreActive(source: file, password: password, directory: storageDirectory())
    }
    func clearBackupFiles() {
        if let pendingBackup { try? FileManager.default.removeItem(at: pendingBackup) }
        if let exportedBackup { try? FileManager.default.removeItem(at: exportedBackup) }
        pendingBackup = nil; exportedBackup = nil
    }
    func readDescriptor(_ url: URL) throws -> String { try readText(url, max: 32_768) }
    func readLabels(_ url: URL) throws -> String { try readText(url, max: 2 * 1024 * 1024) }
    private func readText(_ url: URL, max: Int) throws -> String {
        guard let text = String(data: try readBytes(url, max: max), encoding: .utf8) else { throw CocoaError(.fileReadCorruptFile) }
        return text
    }
    private func readBytes(_ url: URL, max: Int) throws -> Data {
        let accessed = url.startAccessingSecurityScopedResource()
        defer { if accessed { url.stopAccessingSecurityScopedResource() } }
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }
        // Read through EOF within a bound; a short read is not assumed to mean EOF.
        var data = Data()
        while data.count <= max {
            let chunk = try handle.read(upToCount: min(8192, max + 1 - data.count)) ?? Data()
            if chunk.isEmpty { break }
            data.append(chunk)
        }
        guard data.count <= max else {
            throw CocoaError(.fileReadCorruptFile)
        }
        return data
    }
}

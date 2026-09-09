import SwiftUI
import UniformTypeIdentifiers

struct EncryptedBackupDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.data] }
    let source: URL
    init(source: URL) { self.source = source }
    init(configuration: ReadConfiguration) throws { throw BackupAccessError.unreadable }
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        // The model retains this completed ciphertext until the export callback and
        // readback finish. The provider never receives an in-progress database.
        let wrapper = try FileWrapper(url: source, options: [])
        // Reading from a URL retains its staging filename independently of the
        // preferred name. Files used that random name despite defaultFilename.
        wrapper.filename = "tundra-backup.tundra"
        wrapper.preferredFilename = "tundra-backup.tundra"
        return wrapper
    }
}

private enum BackupPasswordField: Hashable { case password, confirmation }

struct BackupView: View {
    @ObservedObject var model: WalletModel
    @Environment(\.dismiss) private var dismiss
    @State private var restoring = false
    @State private var password = ""
    @State private var confirmation = ""
    @State private var acknowledged = false
    @State private var importing = false
    @State private var exporting = false
    @FocusState private var editingPassword: BackupPasswordField?
    private var locked: Bool { model.busy || model.exportedBackup != nil }
    var body: some View {
        NavigationStack {
            Form {
                Picker("Backup action", selection: $restoring) {
                    Text("Export").tag(false).disabled(!model.storageReady)
                    Text("Restore").tag(true)
                }.disabled(locked).accessibilityIdentifier("backupAction")
                Text(restoring
                    ? "Restore public wallets and their private metadata from an encrypted Tundra backup. Hardware signing keys are never in this file."
                    : "Export all wallets, labels, freezes and saved payment history. The file is encrypted with its own password.")
                Text("Use a strong, unique password of at least 16 characters and keep it separately. Spaces and Unicode are significant; a lost password cannot be recovered.")
                SecureField("Backup password", text: $password)
                    .textInputAutocapitalization(.never).autocorrectionDisabled().disabled(locked)
                    .focused($editingPassword, equals: .password)
                    .accessibilityIdentifier("backupPassword")
                if !restoring {
                    SecureField("Confirm backup password", text: $confirmation)
                        .textInputAutocapitalization(.never).autocorrectionDisabled().disabled(locked)
                        .focused($editingPassword, equals: .confirmation)
                        .accessibilityIdentifier("backupPasswordConfirmation")
                    Button("Create encrypted backup") { editingPassword = nil; model.prepareBackup(password: password) }
                        .disabled(locked || !model.storageReady || password.isEmpty || password != confirmation)
                        .accessibilityIdentifier("prepareBackup")
                } else {
                    Button("Choose backup file") { importing = true }.disabled(locked || password.isEmpty)
                    if let info = model.backupPreview {
                        Section("Review backup") {
                            Text(info.createdAt <= 253402300799
                                ? "Created " + Date(timeIntervalSince1970: TimeInterval(info.createdAt)).formatted()
                                : "Creation time unavailable")
                            Text("\(info.wallets.count) wallets · \(info.drafts) saved payments · \(info.submissions) submission records")
                            ForEach(info.wallets, id: \.id) { Text($0.name + " · " + ($0.policy == .singleSig ? "Single signature" : "2 of 3 signatures")) }
                            Text("Balances will be unknown until a new sync. Saved approvals are suspended, and unresolved submission inputs remain held.")
                            Text("An old backup cannot know receive addresses issued later. Sync cannot discover unused addresses, so recovery may reuse one. Compare receive addresses and policies on your hardware.")
                            Text("Restore switches to a new protected store. Existing wallet files and their keys are retained on this device.")
                            Toggle("I understand these recovery limits and want to use this backup", isOn: $acknowledged)
                                .disabled(locked).accessibilityIdentifier("backupRestoreConsent")
                            Button("Restore reviewed backup") {
                                let entered = password; password = ""; confirmation = ""
                                model.restoreBackup(password: entered, acknowledged: acknowledged); acknowledged = false
                            }.disabled(locked || !acknowledged || password.isEmpty).accessibilityIdentifier("restoreBackup")
                        }
                    }
                }
                if model.busy { ProgressView().accessibilityLabel("Working") }
                Text("Keep a verified copy outside this app and device. Deleting the app removes its local files.").font(.footnote)
                if let message = model.backupMessage { Text(message).accessibilityIdentifier("backupResult") }
                if let error = model.error { Text(error).foregroundStyle(.red) }
            }
            .navigationTitle("Backup and recovery")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { password = ""; confirmation = ""; model.cancelBackup(); dismiss() }.disabled(locked)
                }
                ToolbarItemGroup(placement: .keyboard) { Spacer(); Button("Done") { editingPassword = nil } }
            }
        }
        .onAppear { restoring = !model.storageReady || model.backupPreview != nil }
        .onChange(of: restoring) { _, _ in password = ""; confirmation = ""; acknowledged = false; model.cancelBackup() }
        .onChange(of: model.backupPreview) { _, _ in acknowledged = false }
        .onChange(of: model.exportedBackup) { _, file in
            if file != nil { password = ""; confirmation = ""; exporting = true }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.data]) { result in
            if case .success(let url) = result { model.inspectBackupFile(url, password: password) }
        }
        .fileExporter(isPresented: $exporting,
            document: model.exportedBackup.map { EncryptedBackupDocument(source: $0) },
            contentTypes: [.data], defaultFilename: "tundra-backup.tundra") { result in
                switch result {
                case .success(let destination): model.finishBackupExport(destination)
                case .failure: model.failedBackupExport()
                }
            } onCancellation: { model.finishBackupExport(nil) }
        .interactiveDismissDisabled(locked)
        .onDisappear { password = ""; confirmation = ""; if !locked { model.cancelBackup() } }
    }
}

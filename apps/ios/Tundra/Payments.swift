import SwiftUI
import UniformTypeIdentifiers

struct PublicTextExport: FileDocument {
    static var readableContentTypes: [UTType] { [.plainText] }
    var text: String
    init(_ text: String) { self.text = text }
    init(configuration: ReadConfiguration) throws {
        text = String(decoding: configuration.file.regularFileContents ?? Data(), as: UTF8.self)
    }
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: Data(text.utf8))
    }
}

private enum PaymentField: Hashable { case recipient, amount, fee, label }

struct PaymentView: View {
    @ObservedObject var model: WalletModel
    @State private var broadcastTarget: BroadcastTarget?
    @Environment(\.dismiss) private var dismiss
    @State var mode: Int
    @FocusState private var editing: PaymentField?
    @State private var address = ""
    @State private var amount = ""
    @State private var fee = "2"
    @State private var label = ""
    @State private var automatic = false
    @State private var acknowledged = false
    @State private var importingPSBT = false
    @State private var scanningPSBT = false
    @State private var qrEncoding: QrEncoding = .ur
    @State private var importTarget: (walletID: String, draftID: String)?
    var body: some View {
        NavigationStack {
            Form {
                let walletID = model.review?.walletId ?? model.selectedID
                if let wallet = model.wallets.first(where: { $0.id == walletID }) {
                    Text("Network: \(String(describing: wallet.network).capitalized)")
                } else { Text("Network unavailable") }
                if let review = model.review {
                    Section("Saved payment · \(review.state)") {
                        Text(review.label.isEmpty ? "Unlabeled payment" : review.label)
                    }
                    Section("Inputs · \(review.inputs.count)") {
                        ForEach(review.inputs, id: \.outpoint) { input in
                            VStack(alignment: .leading) {
                                Text(input.label.isEmpty ? "Unlabeled coin" : input.label)
                                Text("\(formatBalance(sats: input.sats)) BTC").monospacedDigit()
                                Text(input.outpoint).font(.caption).foregroundStyle(.secondary)
                            }
                        }
                    }
                    Section("Outputs") {
                        ForEach(Array(review.outputs.enumerated()), id: \.offset) { _, output in
                            VStack(alignment: .leading) {
                                Text(output.isChange ? "Change to this wallet" : review.isConsolidation ? "Consolidation to this wallet" : "Recipient")
                                Text("\(formatBalance(sats: output.sats)) BTC").monospacedDigit()
                                Text(output.address).font(.caption)
                            }
                        }
                    }
                    Section("Fee") {
                        Text("\(review.feeSats) sats")
                        if let rate = review.feeSatPerKwu { Text("\(formatFeeRate(satPerKwu: rate)) sat/vB requested") }
                    }
                    Section {
                        Text("Compare every output on your hardware before signing. Hardware signing is not qualified in this build.")
                        if let signing = model.signing, signing.draftId == review.id {
                            Text("Verified signatures").font(.headline)
                            ForEach(Array(signing.inputs.enumerated()), id: \.offset) { index, input in
                                Text("Input \(index + 1): \(input.validSignatures) / \(input.requiredSignatures)")
                            }
                            if signing.complete {
                                Text("All required signatures verified. Finalize to review the exact transaction.")
                                if review.state != "finalized" { Button("Finalize for review") { model.finalizeReview() }.disabled(model.busy) }
                            }
                        }
                        if let finalized = model.finalized, finalized.draftId == review.id {
                            Text("Final transaction").font(.headline)
                            Text(finalized.txid).font(.caption)
                            Text("\(finalized.vsize) vB · \(finalized.feeSats) sats fee")
                            Text("Saved for final review. Inputs remain reserved until synchronization observes the spend.")
                        }
                        if let submission = model.submission, submission.draftId == review.id {
                            Text("Submission").font(.headline)
                            Text(submission.txid).font(.caption)
                            Text(submission.endpoint).font(.caption)
                            Text(submissionMessage(submission))
                            if model.recoveryRequired {
                                RecoveryReviewSection(review: review, submission: submission, synced: model.wallet?.synced == true,
                                    busy: model.busy, resume: model.resumeRecovery)
                                    .id("\(review.walletId):\(review.id):\(submission.attemptId):\(model.wallet?.syncedAt ?? 0)")
                            }
                        }
                        if review.state == "finalized", let transactionID = model.finalized?.txid ?? model.submission?.txid {
                            Button(model.submission == nil ? "Review test-network broadcast" : "Review resubmission") {
                                broadcastTarget = BroadcastTarget(request: BroadcastRequest(walletId: review.walletId, draftId: review.id,
                                    endpoint: model.submission?.endpoint ?? model.endpoint, expectedTxid: transactionID,
                                    previousAttempt: model.submission?.attemptId, privacyConsent: false, retryAcknowledged: false))
                            }.disabled(model.busy).accessibilityIdentifier("reviewBroadcast")
                        }
                        Button(review.state == "unsigned" ? "Export unsigned PSBT" : "Export PSBT with signatures") { model.exportDraft() }.disabled(["invalidated", "observed"].contains(review.state) || model.busy)
                        Button("Import signed PSBT") {
                            importTarget = (review.walletId, review.id); importingPSBT = true
                        }.disabled(["invalidated", "finalized", "observed"].contains(review.state) || model.busy)
                        Picker("QR format", selection: $qrEncoding) { Text("UR").tag(QrEncoding.ur); Text("BBQr").tag(QrEncoding.bbqr) }
                        Button("Show PSBT QR") { model.exportQr(qrEncoding) }.disabled(["invalidated", "observed"].contains(review.state) || model.busy)
                        Button("Scan signed PSBT") {
                            importTarget = (review.walletId, review.id); scanningPSBT = true
                        }.disabled(["invalidated", "finalized", "observed"].contains(review.state) || model.busy)
                        Text("PSBT exports are unencrypted wallet metadata.").font(.caption)
                        Button("Discard draft and release inputs", role: .destructive) { model.discardReview() }.disabled(model.busy || model.submission != nil)
                    }
                } else {
                    Section("Payment") {
                        Picker("Mode", selection: $mode) { Text("Send").tag(0); Text("Max").tag(1); Text("Consolidate").tag(2) }
                            .accessibilityIdentifier("paymentMode")
                        if mode != 2 {
                            TextField("Recipient address", text: $address).textInputAutocapitalization(.never).autocorrectionDisabled()
                                .focused($editing, equals: .recipient)
                        }
                        if mode == 0 {
                            HStack {
                                Text("Amount")
                                TextField("Amount in BTC", text: $amount).keyboardType(.decimalPad)
                                    .focused($editing, equals: .amount)
                                    .multilineTextAlignment(.trailing)
                                Text("BTC").foregroundStyle(.secondary)
                            }
                            Toggle("Automatic eligible inputs", isOn: $automatic).accessibilityIdentifier("automaticInputs")
                        }
                        if !automatic || mode != 0 { Text("\(model.selectedCoins.count) exact inputs selected. Change selection in Coins.") }
                        if mode == 2 {
                            Text("Send selected coins to a fresh internal address in this wallet.")
                            Toggle("I understand this links these coins on-chain", isOn: $acknowledged)
                                .accessibilityIdentifier("consolidationConsent")
                        }
                        HStack {
                            Text("Fee rate")
                            TextField("Fee rate in sat/vB", text: $fee).keyboardType(.decimalPad)
                                .focused($editing, equals: .fee)
                                .multilineTextAlignment(.trailing)
                            Text("sat/vB").foregroundStyle(.secondary)
                        }
                        TextField("Payment label", text: $label).focused($editing, equals: .label)
                        Text("Review reserves inputs and saves an unsigned draft. It does not sign or broadcast.").font(.caption)
                        Button("Review payment") {
                            editing = nil
                            model.createPayment(mode: mode, address: address, amount: amount, fee: fee,
                                label: label, automatic: automatic, acknowledge: acknowledged)
                        }.disabled(model.busy)
                    }
                }
                if let error = model.error { Text(error).foregroundStyle(.red) }
            }
            .navigationTitle("Review payment")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(model.review == nil ? "Close" : "Save for later") { model.closeReview(); dismiss() }.disabled(model.busy)
                }
                ToolbarItemGroup(placement: .keyboard) { Spacer(); Button("Done") { editing = nil } }
            }
        }
        .sheet(item: $broadcastTarget) { target in
            BroadcastView(initial: target.request) { request in
                broadcastTarget = nil
                model.submitBroadcast(request)
            }
        }
        .sheet(isPresented: $scanningPSBT, onDismiss: { importTarget = nil }) {
            QrScanView(purpose: .signedPsbt) { data in
                if let target = importTarget { model.importSignedQr(data, walletID: target.walletID, draftID: target.draftID) }
                scanningPSBT = false
            }
        }
        .sheet(isPresented: Binding(get: { model.qrFrames != nil }, set: { if !$0 { model.qrFrames = nil } })) {
            if let frames = model.qrFrames { QrDisplayView(frames: frames) }
        }
        .onDisappear { model.qrFrames = nil }
        .onAppear { automatic = model.selectedCoins.isEmpty }
        .interactiveDismissDisabled(model.busy)
        .fileExporter(isPresented: Binding(get: { model.exportedPSBT != nil }, set: { if !$0 { model.exportedPSBT = nil } }),
            document: PublicTextExport(model.exportedPSBT ?? ""), contentType: .plainText, defaultFilename: "tundra-signing.psbt.txt") { result in
                if case .failure = result { model.error = "PSBT export could not be saved." }
                model.exportedPSBT = nil
            }
        .fileImporter(isPresented: $importingPSBT, allowedContentTypes: [.data, .plainText]) { result in
            if let target = importTarget {
                switch result {
                case .success(let url): model.importSignedDraft(url, walletID: target.walletID, draftID: target.draftID)
                case .failure: model.error = "Signed PSBT file could not be opened."
                }
            }
            importTarget = nil
        }
    }
}

struct CoinsView: View {
    @ObservedObject var model: WalletModel
    var active: Bool
    var send: (Int) -> Void
    @State private var query = ""
    @State private var availableOnly = false
    @State private var largestFirst = false
    @State private var selecting = false
    @State private var editing: CoinInfo?
    @State private var label = ""
    @State private var bulkLabel = false
    @FocusState private var searching: Bool
    @FocusState private var editingLabel: Bool
    private var visible: [CoinInfo] {
        let filtered = model.coins.filter { coin in
            (!availableOnly || coin.state == .available) &&
            (query.isEmpty || [coin.label, coin.address, coin.outpoint].contains { $0.localizedCaseInsensitiveContains(query) })
        }
        return largestFirst ? filtered.sorted { $0.sats > $1.sats } : filtered.sorted { $0.label.localizedCompare($1.label) == .orderedAscending }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            TextField("Search labels, addresses, outpoints", text: $query).textFieldStyle(.roundedBorder)
                .textInputAutocapitalization(.never).autocorrectionDisabled()
                .focused($searching).submitLabel(.done).onSubmit { searching = false }
            HStack {
                let active = (availableOnly ? 1 : 0) + (largestFirst ? 1 : 0)
                Menu {
                    Toggle("Available only", isOn: $availableOnly)
                    Toggle("Largest first", isOn: $largestFirst)
                } label: { Text(active == 0 ? "Filter and sort" : "Filter and sort · \(active) active") }
                .accessibilityLabel("Filter and sort").accessibilityValue("\(active) active")
                .accessibilityIdentifier("coinFilters")
                Spacer()
                Button(selecting ? "Done selecting" : "Select") {
                    selecting.toggle(); if !selecting { model.selectedCoins = [] }
                }.disabled(model.busy)
            }.font(.caption)
            if !model.selectedCoins.isEmpty {
                let selectedSats = model.coins.filter { model.selectedCoins.contains($0.outpoint) }.reduce(UInt64(0)) { $0 + $1.sats }
                Text("\(model.selectedCoins.count) selected · \(formatBalance(sats: selectedSats)) BTC")
                HStack {
                    Button("Send selected") { send(0) }.buttonStyle(.borderedProminent)
                    Menu("More") {
                        Button("Label selected") { label = ""; bulkLabel = true }
                        Button("Consolidate") { send(2) }.disabled(model.selectedCoins.count < 2)
                        Button("Freeze selected") { model.editCoins(Array(model.selectedCoins), label: nil, frozen: true) }
                        Button("Clear selection") { model.selectedCoins = [] }
                    }.buttonStyle(.bordered)
                }.disabled(model.busy)
            }
            if visible.isEmpty {
                Text(!model.coins.isEmpty ? "No coins match this search and filters."
                    : model.wallet?.synced == true ? "No coins were found at the last successful sync."
                    : "Sync this wallet with your chosen test endpoint to load its coins.")
                    .font(.subheadline).foregroundStyle(.secondary)
            }
            ForEach(visible, id: \.outpoint) { coin in
                HStack(alignment: .top) {
                    if selecting {
                    Button { model.toggleCoin(coin) } label: {
                        Image(systemName: model.selectedCoins.contains(coin.outpoint) ? "checkmark.square.fill" : "square")
                            .frame(width: 44, height: 44)
                    }.disabled(coin.state != .available || model.busy).accessibilityLabel("Select \(coin.label.isEmpty ? "coin" : coin.label)")
                    }
                    Button { label = coin.label; editing = coin; model.loadOutputSource(coin.outpoint) } label: {
                        VStack(alignment: .leading, spacing: 5) {
                            Text(coin.label.isEmpty ? "Add a label" : coin.label).foregroundStyle(.primary)
                            Text(String(describing: coin.state).capitalized).font(.caption).foregroundStyle(.secondary)
                            Text("\(formatBalance(sats: coin.sats)) BTC").monospacedDigit()
                        }.frame(maxWidth: .infinity, alignment: .leading)
                    }.disabled(model.busy).accessibilityIdentifier("coinDetails")
                }
            }
        }
        .onChange(of: active) { _, active in if !active { searching = false } }
        .sheet(isPresented: Binding(get: { editing != nil }, set: { if !$0 { editing = nil } })) {
            VStack(spacing: 20) {
                Text("Coin label").font(.title2)
                TextField("Label", text: $label).textFieldStyle(.roundedBorder)
                    .focused($editingLabel).submitLabel(.done).onSubmit { editingLabel = false }
                if let coin = editing {
                    Text(coin.outpoint).font(.caption)
                    if let source = model.outputSource, model.sourceOutpoint == coin.outpoint, source.walletId == model.selectedID {
                        Text("Created by \(source.label.isEmpty ? "saved payment" : source.label)").font(.headline)
                        Text("Original input labels · historical record").font(.caption)
                        ScrollView {
                            VStack(alignment: .leading, spacing: 8) {
                                ForEach(source.inputs, id: \.outpoint) { input in
                                    Text("\(input.label.isEmpty ? "Unlabeled input" : input.label) · \(formatBalance(sats: input.sats)) BTC").font(.caption)
                                }
                            }.frame(maxWidth: .infinity, alignment: .leading)
                        }.frame(maxHeight: 180)
                    }
                    Button("Save") { model.editCoins([coin.outpoint], label: label, frozen: nil); editing = nil }
                    Button(coin.state == .frozen ? "Unfreeze coin" : "Freeze coin") {
                        model.editCoins([coin.outpoint], label: nil, frozen: coin.state != .frozen); editing = nil
                    }
                }
                Button("Cancel") { editing = nil }
            }.padding(24).presentationDetents([.medium, .large])
        }
        .alert("Label selected coins", isPresented: $bulkLabel) {
            TextField("Label", text: $label)
            Button("Apply") { model.editCoins(Array(model.selectedCoins), label: label, frozen: nil) }
            Button("Cancel", role: .cancel) { }
        }
    }
}

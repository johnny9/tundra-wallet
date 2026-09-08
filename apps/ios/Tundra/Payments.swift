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

struct PaymentView: View {
    @ObservedObject var model: WalletModel
    @Environment(\.dismiss) private var dismiss
    @State var mode: Int
    @State private var address = ""
    @State private var amount = ""
    @State private var fee = "2"
    @State private var label = ""
    @State private var automatic = false
    @State private var acknowledged = false
    var body: some View {
        NavigationStack {
            Form {
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
                        Button("Export unsigned PSBT") { model.exportDraft() }.disabled(review.state != "unsigned" || model.busy)
                        Text("PSBT exports are unencrypted wallet metadata.").font(.caption)
                        Button("Discard draft and release inputs", role: .destructive) { model.discardReview() }.disabled(model.busy)
                    }
                } else {
                    Section("Payment") {
                        Picker("Mode", selection: $mode) { Text("Send").tag(0); Text("Max").tag(1); Text("Consolidate").tag(2) }
                        if mode != 2 {
                            TextField("Recipient address", text: $address).textInputAutocapitalization(.never).autocorrectionDisabled()
                        }
                        if mode == 0 {
                            TextField("Amount in BTC", text: $amount).keyboardType(.decimalPad)
                            Toggle("Automatic eligible inputs", isOn: $automatic)
                        }
                        if !automatic || mode != 0 { Text("\(model.selectedCoins.count) exact inputs selected. Change selection in Coins.") }
                        if mode == 2 {
                            Text("Send selected coins to a fresh internal address in this wallet.")
                            Toggle("I understand this links these coins on-chain", isOn: $acknowledged)
                                .accessibilityIdentifier("consolidationConsent")
                        }
                        TextField("Fee rate in sat/vB", text: $fee).keyboardType(.decimalPad)
                        TextField("Payment label", text: $label)
                        Text("Review reserves inputs and saves an unsigned draft. It does not sign or broadcast.").font(.caption)
                        Button("Review payment") {
                            model.createPayment(mode: mode, address: address, amount: amount, fee: fee,
                                label: label, automatic: automatic, acknowledge: acknowledged)
                        }.disabled(model.busy)
                    }
                }
                if let error = model.error { Text(error).foregroundStyle(.red) }
            }
            .navigationTitle("Review payment")
            .toolbar { ToolbarItem(placement: .cancellationAction) {
                Button(model.review == nil ? "Close" : "Save for later") { model.review = nil; dismiss() }.disabled(model.busy)
            } }
        }
        .onAppear { automatic = model.selectedCoins.isEmpty }
        .interactiveDismissDisabled(model.busy)
        .fileExporter(isPresented: Binding(get: { model.exportedPSBT != nil }, set: { if !$0 { model.exportedPSBT = nil } }),
            document: PublicTextExport(model.exportedPSBT ?? ""), contentType: .plainText, defaultFilename: "tundra-unsigned.psbt.txt") { result in
                if case .failure = result { model.error = "PSBT export could not be saved." }
                model.exportedPSBT = nil
            }
    }
}

struct CoinsView: View {
    @ObservedObject var model: WalletModel
    var send: (Int) -> Void
    @State private var query = ""
    @State private var availableOnly = false
    @State private var largestFirst = false
    @State private var editing: CoinInfo?
    @State private var label = ""
    @State private var bulkLabel = false
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
            HStack {
                Toggle("Available", isOn: $availableOnly)
                Toggle("Largest first", isOn: $largestFirst)
            }.font(.caption)
            if !model.selectedCoins.isEmpty {
                Text("\(model.selectedCoins.count) selected")
                HStack {
                    Button("Send") { send(0) }
                    Button("Consolidate") { send(2) }.disabled(model.selectedCoins.count < 2)
                    Button("Label") { label = ""; bulkLabel = true }
                }
                HStack {
                    Button("Freeze selected") { model.editCoins(Array(model.selectedCoins), label: nil, frozen: true) }
                    Button("Clear selection") { model.selectedCoins = [] }
                }.font(.caption)
            }
            ForEach(visible, id: \.outpoint) { coin in
                HStack(alignment: .top) {
                    Button { model.toggleCoin(coin) } label: {
                        Image(systemName: model.selectedCoins.contains(coin.outpoint) ? "checkmark.square.fill" : "square")
                            .frame(width: 44, height: 44)
                    }.disabled(coin.state != .available || model.busy).accessibilityLabel("Select \(coin.label.isEmpty ? "coin" : coin.label)")
                    Button { label = coin.label; editing = coin } label: {
                        VStack(alignment: .leading, spacing: 5) {
                            Text(coin.label.isEmpty ? "Add a label" : coin.label).foregroundStyle(.primary)
                            Text(String(describing: coin.state).capitalized).font(.caption).foregroundStyle(.secondary)
                            Text("\(formatBalance(sats: coin.sats)) BTC").monospacedDigit()
                        }.frame(maxWidth: .infinity, alignment: .leading)
                    }.disabled(model.busy)
                }
            }
        }
        .sheet(isPresented: Binding(get: { editing != nil }, set: { if !$0 { editing = nil } })) {
            VStack(spacing: 20) {
                Text("Coin label").font(.title2)
                TextField("Label", text: $label).textFieldStyle(.roundedBorder)
                if let coin = editing {
                    Text(coin.outpoint).font(.caption)
                    Button("Save") { model.editCoins([coin.outpoint], label: label, frozen: nil); editing = nil }
                    Button(coin.state == .frozen ? "Unfreeze coin" : "Freeze coin") {
                        model.editCoins([coin.outpoint], label: nil, frozen: coin.state != .frozen); editing = nil
                    }
                }
                Button("Cancel") { editing = nil }
            }.padding(24).presentationDetents([.medium])
        }
        .alert("Label selected coins", isPresented: $bulkLabel) {
            TextField("Label", text: $label)
            Button("Apply") { model.editCoins(Array(model.selectedCoins), label: label, frozen: nil) }
            Button("Cancel", role: .cancel) { }
        }
    }
}

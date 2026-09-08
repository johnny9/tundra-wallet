import SwiftUI
import UniformTypeIdentifiers

@main
struct TundraApp: App {
    @StateObject private var model = WalletModel()
    @AppStorage("tundra.dark") private var dark = true
    @Environment(\.scenePhase) private var phase
    var body: some Scene {
        WindowGroup {
            WalletView(model: model, dark: $dark)
                .preferredColorScheme(dark ? .dark : .light)
                .tint(Color(red: 0.973, green: 0.608, blue: 0.165))
                .privacySensitive()
                .blur(radius: phase == .active ? 0 : 18)
                .task { model.load() }
                .onChange(of: phase) { _, phase in if phase != .active { model.cancelSync() } }
        }
    }
}

struct WalletView: View {
    @ObservedObject var model: WalletModel
    @Binding var dark: Bool
    @State private var tab = 0
    @State private var adding = false
    @State private var syncing = false
    private var background: Color { dark ? Color(red: 0.055, green: 0.067, blue: 0.082) : Color(red: 0.984, green: 0.988, blue: 0.992) }
    var body: some View {
        NavigationStack {
            VStack(spacing: 20) {
                HStack {
                    TundraMark().frame(width: 24, height: 24).accessibilityHidden(true)
                    Menu {
                        ForEach(model.wallets, id: \.id) { w in Button(w.name) { model.select(w.id) } }
                        Button("Add wallet", systemImage: "plus") { adding = true }
                    } label: {
                        HStack { Text(model.wallet?.name ?? "Tundra").font(.headline); Image(systemName: "chevron.down").font(.caption) }
                    }.foregroundStyle(.primary)
                    Spacer()
                    Menu {
                        Toggle("Dark appearance", isOn: $dark)
                        Text("Development build · test descriptors only")
                    } label: { Image(systemName: "gearshape").frame(width: 44, height: 44) }
                    .accessibilityLabel("Settings")
                }
                VStack(alignment: .leading, spacing: 6) {
                    Text(model.wallet?.totalSats.map { formatBalance(sats: $0) + " BTC" } ?? "— BTC")
                        .font(.system(size: 32, weight: .medium, design: .rounded)).monospacedDigit()
                        .accessibilityIdentifier("balance")
                    Text(model.wallet == nil ? "Your bitcoin. Your hardware." : model.wallet?.syncedAt.map { "Cached · last sync \(Date(timeIntervalSince1970: TimeInterval($0)).formatted())" } ?? "Not synced · balance unknown")
                        .font(.subheadline).foregroundStyle(.secondary)
                    if model.wallet != nil {
                        HStack {
                            Button("Sync test network") { syncing = true }.disabled(model.busy)
                            if let sync = model.sync, [.prepared, .scanning, .applying].contains(sync.state) {
                                Text("\(sync.scannedScripts) scripts").font(.caption)
                                Button("Cancel sync") { model.cancelSync() }
                            }
                        }
                    }
                }.frame(maxWidth: .infinity, alignment: .leading)
                Picker("Wallet view", selection: $tab) { Text("Activity").tag(0); Text("Coins").tag(1) }
                    .pickerStyle(.segmented)
                if model.wallet == nil {
                    ContentUnavailableView("Add a wallet", systemImage: "wallet.pass",
                        description: Text("Import a public test descriptor. Private signing keys stay on hardware."))
                    Button("Import descriptor") { adding = true }.buttonStyle(.borderedProminent)
                } else {
                    if tab == 0 {
                        HStack {
                            Button("Receive", systemImage: "arrow.down") { model.receive() }.buttonStyle(.bordered)
                            Button("Send", systemImage: "arrow.up") { }.buttonStyle(.borderedProminent).disabled(true)
                        }.frame(maxWidth: .infinity)
                    }
                    ScrollView {
                        VStack(alignment: .leading, spacing: 16) {
                            if tab == 0 {
                                ForEach(model.activity, id: \.txid) { row in
                                    VStack(alignment: .leading) {
                                        Text(row.label.isEmpty ? "Unlabeled transaction" : row.label)
                                        Text(row.confirmed ? "Confirmed" : "Pending").font(.caption).foregroundStyle(.secondary)
                                    }
                                }
                            } else {
                                ForEach(model.coins, id: \.outpoint) { coin in
                                    HStack {
                                        Text(coin.label.isEmpty ? "Unlabeled coin" : coin.label)
                                        Spacer(); Text(formatBalance(sats: coin.sats)).monospacedDigit()
                                    }
                                }
                            }
                            Text("Hardware signing is not qualified. Use disposable test wallets only.")
                                .font(.footnote).foregroundStyle(.secondary)
                        }.frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
                Spacer(minLength: 0)
                if model.busy { ProgressView().accessibilityLabel("Working") }
                if let error = model.error { Text(error).font(.footnote).foregroundStyle(.red) }
                Text("Development only. Do not fund these addresses.").font(.caption).foregroundStyle(.secondary)
            }
            .padding(24).background(background.ignoresSafeArea())
            .sheet(isPresented: $adding, onDismiss: { model.cancelImport() }) { ImportView(model: model) }
            .sheet(isPresented: $syncing) { SyncView(model: model) }
            .sheet(isPresented: Binding(get: { model.received != nil }, set: { if !$0 { model.received = nil } })) {
                VStack(alignment: .leading, spacing: 20) {
                    Text("Unverified address").font(.title2)
                    Text(model.received?.address ?? "").font(.system(.body, design: .monospaced))
                    Text("Receive index \(model.received?.index ?? 0)").accessibilityIdentifier("receiveIndex")
                    Text("This address has not been verified on hardware. This build is for disposable test descriptors only. Do not fund it.")
                    Button("Close") { model.received = nil }
                }.padding(24).presentationDetents([.medium])
            }
        }
    }
}

struct SyncView: View {
    @ObservedObject var model: WalletModel
    @Environment(\.dismiss) private var dismiss
    @State private var endpoint = ""
    @State private var consent = false
    var body: some View {
        NavigationStack {
            Form {
                TextField("Esplora API URL", text: $endpoint)
                    .textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
                Text("Your server can associate requested script hashes and transactions with your IP address. Descriptors and labels stay on this device. The server supplies your view of the test chain.")
                Toggle("I trust this endpoint and agree to these requests", isOn: $consent)
                Button("Start scan") { model.synchronize(endpoint, consent: consent); dismiss() }
                    .disabled(!consent || endpoint.isEmpty || model.busy)
            }
            .navigationTitle("Test endpoint")
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        }.onAppear { endpoint = model.endpoint }
    }
}

struct ImportView: View {
    @ObservedObject var model: WalletModel
    @Environment(\.dismiss) private var dismiss
    @State private var payload = ""
    @State private var name = "Savings"
    @State private var chain: Chain = .signet
    @State private var filePicker = false
    var body: some View {
        NavigationStack {
            Form {
                Section("Public test descriptor") {
                    Picker("Network", selection: $chain) { Text("Signet").tag(Chain.signet); Text("Regtest").tag(Chain.regtest) }
                    Button("Import descriptor file") { filePicker = true }
                    TextEditor(text: $payload).font(.system(.caption, design: .monospaced))
                        .frame(minHeight: 120).autocorrectionDisabled().textInputAutocapitalization(.never)
                        .accessibilityLabel("Public descriptor")
                        .accessibilityIdentifier("publicDescriptor")
                    Button("Review descriptor") { model.inspect(payload, chain: chain) }.disabled(payload.isEmpty || model.busy)
                    Text("QR scanning is planned. This native build does not simulate it.").font(.caption)
                }
                if let preview = model.preview {
                    Section("Review") {
                        Text(preview.policy == .singleSig ? "Single signature" : "2 of 3 multisig")
                        TextField("Wallet name", text: $name)
                        Text("Unverified first address").font(.caption)
                        Text(preview.firstAddress).font(.system(.caption, design: .monospaced))
                        Button("Add wallet") { model.add(name) }.disabled(name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.busy)
                    }
                }
                if let error = model.error { Text(error).foregroundStyle(.red) }
                Text("Use public test fixtures only. No private keys, real coins or recoverable backups.").font(.footnote)
            }
            .navigationTitle("Add a wallet")
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } } }
            .fileImporter(isPresented: $filePicker, allowedContentTypes: [.plainText, .json, .data]) { result in
                if case let .success(url) = result { model.inspectFile(url, chain: chain) }
            }
        }
    }
}

struct TundraMark: View {
    var body: some View {
        Canvas { context, size in
            let x = size.width / 24
            let y = size.height / 24
            var path = Path()
            path.move(to: CGPoint(x: 3*x, y: 12*y))
            path.addLine(to: CGPoint(x: 7*x, y: 12*y))
            path.addLine(to: CGPoint(x: 11*x, y: 7*y))
            path.addLine(to: CGPoint(x: 15*x, y: 12*y))
            path.addLine(to: CGPoint(x: 21*x, y: 12*y))
            path.move(to: CGPoint(x: 5*x, y: 17*y))
            path.addLine(to: CGPoint(x: 19*x, y: 17*y))
            path.move(to: CGPoint(x: 8*x, y: 21*y))
            path.addLine(to: CGPoint(x: 16*x, y: 21*y))
            context.stroke(path, with: .color(Color(red: 0.973, green: 0.608, blue: 0.165)),
                           style: StrokeStyle(lineWidth: 1.8*x, lineCap: .round, lineJoin: .round))
        }
    }
}

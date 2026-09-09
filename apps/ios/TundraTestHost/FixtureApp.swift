// This entry point and the published fixtures belong only to TundraTestHost.
// The production Tundra target does not compile or bundle them.
import SwiftUI

private enum FixtureFailure: Error { case preparation }

@main
@MainActor
struct FixtureApp: App {
    @StateObject private var model: WalletModel
    private let prepared: Bool
    init() {
        let service: CoreService?
        do { service = try Self.prepare() } catch { service = nil }
        prepared = service != nil
        _model = StateObject(wrappedValue: WalletModel(service: service ?? CoreService()))
    }
    var body: some Scene {
        WindowGroup {
            if prepared {
                FixtureView(model: model, layout: ProcessInfo.processInfo.environment["TUNDRA_PUBLIC_UI_SCENARIO"] == "layout")
            }
            else { Text("Public fixture preparation failed") }
        }
    }
    private static func prepare() throws -> CoreService {
        let environment = ProcessInfo.processInfo.environment
        guard let id = environment["TUNDRA_PUBLIC_UI_ID"].flatMap(UUID.init(uuidString:)),
              let scenario = environment["TUNDRA_PUBLIC_UI_SCENARIO"], ["signing", "recovery", "layout"].contains(scenario) else {
            throw FixtureFailure.preparation
        }
        let manager = FileManager.default
        let parent = try manager.url(for: .applicationSupportDirectory, in: .userDomainMask,
            appropriateFor: nil, create: true).appendingPathComponent("PublicUITests", isDirectory: true)
        let root = parent.appendingPathComponent(id.uuidString, isDirectory: true)
        let marker = root.appendingPathComponent("public-fixture.v1")
        let service = "dev.johnny9.tundra.public-ui." + id.uuidString
        let expected = Data(scenario.utf8)
        if manager.fileExists(atPath: root.path) {
            // Relaunch only an already prepared fixture. Never reset a partial or
            // mismatched store to make a UI test pass.
            guard try Data(contentsOf: marker) == expected else { throw FixtureFailure.preparation }
        } else {
            try manager.createDirectory(at: root, withIntermediateDirectories: true,
                attributes: [.protectionKey: FileProtectionType.complete])
            if scenario == "signing" {
                guard let source = Bundle.main.url(forResource: "native-unsigned", withExtension: "sqlite") else { throw FixtureFailure.preparation }
                try manager.copyItem(at: source, to: root.appendingPathComponent("wallet.sqlite"))
            } else if scenario == "layout" {
                guard let source = Bundle.main.url(forResource: "two-of-three", withExtension: "txt") else { throw FixtureFailure.preparation }
                let core = try StorageVault.openActive(directory: root, service: service)
                _ = try core.importWallet(name: "Public long-list wallet", payload: String(contentsOf: source, encoding: .utf8), network: .regtest)
            } else {
                guard let source = Bundle.main.url(forResource: "native-signed-backup", withExtension: "tundra") else { throw FixtureFailure.preparation }
                // This is setup. The normal app's separate UI test covers the real
                // backup provider, password, review and generation-switch controls.
                let restored = try StorageVault.restoreActive(source: source, password: "Public native signing backup 2026",
                    directory: root, service: service)
                guard try restored.wallets().count == 1 else { throw FixtureFailure.preparation }
            }
            try expected.write(to: marker, options: .atomic)
        }
        return CoreService(directory: root, keychainService: service)
    }
}

@MainActor
private struct FixtureView: View {
    @ObservedObject var model: WalletModel
    @State private var loaded = false
    @State private var wallet = false
    @State private var dark = true
    @State private var largeText = false
    private let layout: Bool
    init(model: WalletModel, layout: Bool) {
        self.model = model; self.layout = layout
        _wallet = State(initialValue: layout)
    }
    var body: some View {
        VStack(spacing: 4) {
            if wallet {
                WalletView(model: model, dark: $dark)
                    .dynamicTypeSize(largeText ? .accessibility3 : .large)
            }
            else { PaymentView(model: model, mode: 0) }
            // Only these public-fixture handoff controls are test-specific. Payment,
            // recovery, broadcast and coin details above are the production views.
            if layout {
                HStack {
                    Button("Sync public fixture") { model.synchronize("http://127.0.0.1:3002", consent: true) }
                    Button("Toggle fixture appearance") { dark.toggle() }
                    Button(largeText ? "Large text enabled" : "Use large text") { largeText = true }
                        .disabled(largeText)
                }.font(.system(size: 12)).disabled(!loaded || model.busy)
            } else { HStack {
                Button("Supply published response") {
                    if let review = model.review,
                       let source = Bundle.main.url(forResource: "native-signed-response", withExtension: "psbt") {
                        model.importSignedDraft(source, walletID: review.walletId, draftID: review.id)
                    }
                }.disabled(!loaded || model.busy || model.review == nil)
                Button("Sync public fixture") { model.synchronize("http://127.0.0.1:3004", consent: true) }
                    .disabled(!loaded || model.busy)
                Button(wallet ? "Show review" : "Show wallet") {
                    if wallet, let draft = model.drafts.first { model.openReview(draft) }
                    else { model.closeReview() }
                    wallet.toggle()
                }.disabled(!loaded || model.busy)
            }.font(.caption).padding(.horizontal) }
            if !loaded || model.busy { Text("Public fixture working") }
            else { Text(model.error == nil ? "Public fixture ready" : "Public fixture operation failed") }
        }
        .preferredColorScheme(dark ? .dark : .light)
        .tint(TundraColors.accent(dark: dark))
        .task {
            model.load()
            while model.busy { try? await Task.sleep(for: .milliseconds(25)) }
            if let draft = model.drafts.first { model.openReview(draft) }
            while model.busy { try? await Task.sleep(for: .milliseconds(25)) }
            loaded = true
        }
    }
}

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
                .tint(TundraColors.accent(dark: dark))
                .privacySensitive()
                .blur(radius: phase == .active ? 0 : 18)
                .task { model.load() }
                .onChange(of: phase) { _, phase in if phase != .active { model.cancelSync() } }
        }
    }
}

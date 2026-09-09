import SwiftUI

// The same semantic orange/neutral pairs as the approved editable design.
enum TundraColors {
    private static func rgb(_ value: UInt32) -> Color {
        Color(red: Double((value >> 16) & 255) / 255,
              green: Double((value >> 8) & 255) / 255,
              blue: Double(value & 255) / 255)
    }
    static func accent(dark: Bool) -> Color { rgb(dark ? 0xF89B2A : 0x975000) }
    static func primary(dark: Bool, pressed: Bool) -> Color {
        rgb(dark ? (pressed ? 0xFFAD4A : 0xF89B2A) : (pressed ? 0xF89B2A : 0xF7931A))
    }
    static let onPrimary = rgb(0x171717)
    static func secondary(dark: Bool) -> Color { rgb(dark ? 0xADB7C3 : 0x566372) }
    static func danger(dark: Bool) -> Color { rgb(dark ? 0xFF9696 : 0xB72C2C) }
}

struct TundraSecondaryStyle: ShapeStyle {
    func resolve(in environment: EnvironmentValues) -> Color {
        TundraColors.secondary(dark: environment.colorScheme == .dark)
    }
}

struct TundraPrimaryButtonStyle: ButtonStyle {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.isEnabled) private var enabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(TundraColors.onPrimary)
            .padding(.horizontal, 16).padding(.vertical, 10)
            .frame(minHeight: 44)
            .background(TundraColors.primary(dark: scheme == .dark, pressed: configuration.isPressed), in: Capsule())
            .contentShape(Capsule())
            .opacity(enabled ? 1 : 0.45)
    }
}

struct TundraErrorText: View {
    let error: String
    @Environment(\.colorScheme) private var scheme
    var body: some View { Text(error).foregroundStyle(TundraColors.danger(dark: scheme == .dark)) }
}

import SwiftUI

struct BroadcastTarget: Identifiable {
    let id = UUID()
    let request: BroadcastRequest
}
func submissionMessage(_ info: BroadcastInfo) -> String {
    switch info.observation {
    case .confirmed: return "Confirmed in the last wallet sync."
    case .mempool: return "Seen unconfirmed in the last wallet sync."
    case .notSeen:
        return info.acknowledged
            ? "The endpoint acknowledged receipt. The transaction has not been observed by wallet sync."
            : "Submission result uncertain. It may have been broadcast. Sync before considering a retry."
    }
}
struct BroadcastView: View {
    let initial: BroadcastRequest
    var submit: (BroadcastRequest) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var endpoint = ""
    @State private var consent = false
    @State private var retry = false
    @FocusState private var editing: Bool
    var body: some View {
        NavigationStack {
            Form {
                Section("Final transaction") { Text(initial.expectedTxid).font(.caption) }
                Section("Endpoint") {
                    TextField("Esplora endpoint", text: $endpoint).textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
                        .focused($editing).accessibilityIdentifier("broadcastEndpoint")
                        .onChange(of: endpoint) { _, _ in consent = false; retry = false }
                    Text("The endpoint receives the complete transaction and can associate it with your connection. Submission cannot be recalled.")
                    Toggle("I approve this transaction and this endpoint", isOn: $consent).accessibilityIdentifier("broadcastConsent")
                }
                if initial.previousAttempt != nil {
                    Section("Previous attempt") {
                        Text("An earlier attempt may already have reached the network. This sends the same bytes again.")
                        Toggle("I checked the latest sync and explicitly choose to resubmit", isOn: $retry).accessibilityIdentifier("broadcastRetryConsent")
                    }
                }
                Button("Submit exact transaction") {
                    var request = initial
                    request.endpoint = endpoint; request.privacyConsent = consent; request.retryAcknowledged = retry
                    submit(request)
                }.disabled(endpoint.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !consent || (initial.previousAttempt != nil && !retry))
                    .accessibilityIdentifier("confirmBroadcast")
            }
            .navigationTitle("Test-network broadcast")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItemGroup(placement: .keyboard) { Spacer(); Button("Done") { editing = false } }
            }
        }.onAppear { endpoint = initial.endpoint }
    }
}

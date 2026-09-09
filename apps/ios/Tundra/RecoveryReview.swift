import SwiftUI

struct RecoveryReviewSection: View {
    let review: PaymentReview
    let submission: BroadcastInfo
    let synced: Bool
    let busy: Bool
    let resume: (RecoveryReviewRequest) -> Void
    @State private var acknowledged = false
    var body: some View {
        Text("Restored submission").font(.headline)
        Text("This transaction may already have been sent. Its inputs remain held while its saved approval is suspended.")
        Text("Sync the wallet, then check the inputs, outputs and fee above. Resume revalidates the exact saved transaction; broadcasting requires a separate confirmation.")
        Toggle("I reviewed this recovered payment and its previous submission", isOn: $acknowledged)
            .disabled(!synced || busy).accessibilityIdentifier("recoveryConsent")
        Button("Resume reviewed payment") {
            let request = RecoveryReviewRequest(walletId: review.walletId, draftId: review.id,
                expectedTxid: submission.txid, expectedAttempt: submission.attemptId, reviewAcknowledged: acknowledged)
            acknowledged = false; resume(request)
        }.disabled(!acknowledged || !synced || busy).accessibilityIdentifier("resumeRecovery")
    }
}

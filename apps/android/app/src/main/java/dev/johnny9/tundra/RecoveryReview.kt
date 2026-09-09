package dev.johnny9.tundra

import androidx.compose.foundation.layout.Row
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import dev.johnny9.tundra.generated.*

@Composable internal fun RecoveryReviewSection(review: PaymentReview, submission: BroadcastInfo,
    synced: Boolean, busy: Boolean, resume: (RecoveryReviewRequest) -> Unit) {
    var acknowledged by remember(review.walletId, review.id, submission.attemptId, synced) { mutableStateOf(false) }
    Text("Restored submission", style = MaterialTheme.typography.titleMedium)
    Text("This transaction may already have been sent. Its inputs remain held while its saved approval is suspended.")
    Text("Sync the wallet, then check the inputs, outputs and fee above. Resume revalidates the exact saved transaction; broadcasting requires a separate confirmation.")
    Row(verticalAlignment = Alignment.CenterVertically) {
        Checkbox(acknowledged, { acknowledged = it }, enabled = synced && !busy,
            modifier = Modifier.testTag("recoveryConsent"))
        Text("I reviewed this recovered payment and its previous submission")
    }
    Button(onClick = {
        val request = RecoveryReviewRequest(review.walletId, review.id, submission.txid, submission.attemptId, acknowledged)
        acknowledged = false; resume(request)
    }, enabled = acknowledged && synced && !busy, modifier = Modifier.testTag("resumeRecovery")) {
        Text("Resume reviewed payment")
    }
}

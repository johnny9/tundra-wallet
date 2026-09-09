@file:OptIn(ExperimentalUnsignedTypes::class)
package dev.johnny9.tundra

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import dev.johnny9.tundra.generated.*

fun submissionMessage(info: BroadcastInfo): String = when (info.observation) {
    ChainObservation.CONFIRMED -> "Confirmed in the last wallet sync."
    ChainObservation.MEMPOOL -> "Seen unconfirmed in the last wallet sync."
    ChainObservation.NOT_SEEN -> if (info.acknowledged)
        "The endpoint acknowledged receipt. The transaction has not been observed by wallet sync."
    else "Submission result uncertain. It may have been broadcast. Sync before considering a retry."
}

@Composable fun BroadcastDialog(initial: BroadcastRequest, onClose: () -> Unit, onSubmit: (BroadcastRequest) -> Unit) {
    val keyboard = LocalSoftwareKeyboardController.current
    var endpoint by remember { mutableStateOf(initial.endpoint) }
    var consent by remember { mutableStateOf(false) }
    var retry by remember { mutableStateOf(false) }
    AlertDialog(onDismissRequest = onClose, title = { Text("Test-network broadcast") }, text = {
        Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text("Submit the exact finalized transaction:")
            Text(initial.expectedTxid, style = MaterialTheme.typography.bodySmall)
            OutlinedTextField(endpoint, { endpoint = it; consent = false; retry = false }, label = { Text("Esplora endpoint") }, modifier = Modifier.testTag("broadcastEndpoint"), singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { keyboard?.hide() }))
            Text("The endpoint receives the complete transaction and can associate it with your connection. Submission cannot be recalled.")
            Row { Checkbox(consent, { consent = it }, modifier = Modifier.testTag("broadcastConsent")); Text("I approve this transaction and this endpoint") }
            if (initial.previousAttempt != null) {
                Text("An earlier attempt may already have reached the network. This sends the same bytes again.")
                Row { Checkbox(retry, { retry = it }, modifier = Modifier.testTag("broadcastRetryConsent")); Text("I checked the latest sync and explicitly choose to resubmit") }
            }
        }
    }, confirmButton = {
        TextButton(onClick = { onSubmit(initial.copy(endpoint = endpoint, privacyConsent = consent, retryAcknowledged = retry)) },
            enabled = endpoint.isNotBlank() && consent && (initial.previousAttempt == null || retry), modifier = Modifier.testTag("confirmBroadcast")) { Text("Submit exact transaction") }
    }, dismissButton = { TextButton(onClick = onClose) { Text("Cancel") } })
}

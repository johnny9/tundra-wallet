@file:OptIn(ExperimentalUnsignedTypes::class, androidx.compose.material3.ExperimentalMaterial3Api::class)
package dev.johnny9.tundra

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import dev.johnny9.tundra.generated.*

@Composable fun PaymentSheet(vm: WalletViewModel, s: WalletState, initialMode: Int, onClose: () -> Unit) {
    var mode by remember { mutableIntStateOf(initialMode) }
    var address by remember { mutableStateOf("") }
    var amount by remember { mutableStateOf("") }
    var fee by remember { mutableStateOf("2") }
    var label by remember { mutableStateOf("") }
    var automatic by remember { mutableStateOf(s.selected.isEmpty()) }
    var acknowledged by remember { mutableStateOf(false) }
    var exportTarget by remember { mutableStateOf<Pair<String, String>?>(null) }
    var importTarget by remember { mutableStateOf<Pair<String, String>?>(null) }
    var scanTarget by remember { mutableStateOf<Pair<String, String>?>(null) }
    var qrEncoding by remember { mutableStateOf(QrEncoding.UR) }
    val export = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("text/plain")) { uri ->
        val target = exportTarget
        if (uri != null && target != null) vm.exportDraft(uri, target.first, target.second)
        exportTarget = null
    }
    val importSigned = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        val target = importTarget
        if (uri != null && target != null) vm.importSignedDraft(uri, target.first, target.second)
        importTarget = null
    }
    ModalBottomSheet(onDismissRequest = { if (!s.busy) onClose() }, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.padding(24.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            val review = s.review
            if (review == null) {
                Text("Create a payment", style = MaterialTheme.typography.headlineSmall)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf("Send", "Max", "Consolidate").forEachIndexed { index, title ->
                        FilterChip(selected = mode == index, onClick = { mode = index }, label = { Text(title) }, enabled = !s.busy)
                    }
                }
                if (mode != 2) OutlinedTextField(address, { address = it }, label = { Text("Recipient address") }, modifier = Modifier.fillMaxWidth())
                if (mode == 0) {
                    OutlinedTextField(amount, { amount = it }, label = { Text("Amount in BTC") }, modifier = Modifier.fillMaxWidth())
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Switch(automatic, { automatic = it }); Text("Automatic eligible inputs")
                    }
                }
                if (!automatic || mode != 0) Text("${s.selected.size} exact inputs selected. Change selection in Coins.")
                if (mode == 2) {
                    Text("Consolidation sends the selected coins to a fresh internal address in this wallet.")
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(acknowledged, { acknowledged = it }); Text("I understand this links these coins on-chain")
                    }
                }
                OutlinedTextField(fee, { fee = it }, label = { Text("Fee rate in sat/vB") }, modifier = Modifier.fillMaxWidth())
                OutlinedTextField(label, { label = it }, label = { Text("Payment label") }, modifier = Modifier.fillMaxWidth())
                Text("Review reserves the selected inputs and saves an unsigned draft. It does not sign or broadcast.", style = MaterialTheme.typography.bodySmall)
                Button(onClick = { vm.createPayment(mode, address, amount, fee, label, automatic, acknowledged) }, enabled = !s.busy, modifier = Modifier.testTag("buildReview")) { Text("Review payment") }
            } else {
                Text("Review payment", style = MaterialTheme.typography.headlineSmall)
                Text(review.label.ifBlank { "Unlabeled payment" }, style = MaterialTheme.typography.titleMedium)
                Text("State: ${review.state}")
                Text("Inputs · ${review.inputs.size}", style = MaterialTheme.typography.titleMedium)
                review.inputs.forEach { input ->
                    Text(input.label.ifBlank { "Unlabeled coin" })
                    Text("${formatBalance(input.sats)} BTC · ${input.outpoint}", style = MaterialTheme.typography.bodySmall)
                }
                HorizontalDivider()
                Text("Outputs", style = MaterialTheme.typography.titleMedium)
                review.outputs.forEach { output ->
                    Text(if (output.isChange) "Change to this wallet" else if (review.isConsolidation) "Consolidation to this wallet" else "Recipient")
                    Text("${formatBalance(output.sats)} BTC", style = MaterialTheme.typography.titleMedium)
                    Text(output.address, style = MaterialTheme.typography.bodySmall)
                }
                Text("Fee: ${review.feeSats} sats${review.feeSatPerKwu?.let { " · ${formatFeeRate(it)} sat/vB requested" } ?: ""}")
                Text("Compare every output on your hardware before signing. Hardware signing is not qualified in this build.", style = MaterialTheme.typography.bodySmall)
                s.signing?.takeIf { it.draftId == review.id }?.let { signing ->
                    Text("Verified signatures", style = MaterialTheme.typography.titleMedium)
                    signing.inputs.forEachIndexed { index, input ->
                        Text("Input ${index + 1}: ${input.validSignatures} / ${input.requiredSignatures}")
                    }
                    if (signing.complete) Text("All required signatures verified. This payment has not been broadcast.")
                }
                OutlinedButton(onClick = { exportTarget = review.walletId to review.id; export.launch("tundra-signing.psbt.txt") }, enabled = !s.busy && review.state != "invalidated") { Text(if (review.state == "unsigned") "Export unsigned PSBT" else "Export PSBT with signatures") }
                OutlinedButton(onClick = { importTarget = review.walletId to review.id; importSigned.launch(arrayOf("*/*")) }, enabled = !s.busy && review.state != "invalidated") { Text("Import signed PSBT") }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf(QrEncoding.UR to "UR", QrEncoding.BBQR to "BBQr").forEach { (encoding, title) ->
                        FilterChip(selected = qrEncoding == encoding, onClick = { qrEncoding = encoding }, label = { Text(title) }, enabled = !s.busy)
                    }
                }
                OutlinedButton(onClick = { vm.exportQr(qrEncoding) }, enabled = !s.busy && review.state != "invalidated") { Text("Show PSBT QR") }
                OutlinedButton(onClick = { scanTarget = review.walletId to review.id }, enabled = !s.busy && review.state != "invalidated") { Text("Scan signed PSBT") }
                Text("PSBT exports are unencrypted wallet metadata.", style = MaterialTheme.typography.bodySmall)
                TextButton(onClick = vm::discardReview, enabled = !s.busy) { Text("Discard draft and release inputs") }
            }
            s.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            TextButton(onClick = onClose, enabled = !s.busy) { Text(if (s.review == null) "Close" else "Save for later") }
            Spacer(Modifier.height(24.dp))
        }
    }
    scanTarget?.let { target ->
        QrScanDialog(QrPurpose.SignedPsbt, onPayload = { payload ->
            scanTarget = null; vm.importSignedQr(payload, target.first, target.second)
        }, onClose = { scanTarget = null })
    }
    s.qrFrames?.let { QrDisplayDialog(it, onClose = vm::closeQr) }
}

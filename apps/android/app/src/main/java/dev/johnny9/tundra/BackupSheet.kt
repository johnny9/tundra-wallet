@file:OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
package dev.johnny9.tundra

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp

@Composable internal fun BackupSheet(vm: WalletViewModel, state: WalletState, onClose: () -> Unit) {
    var restoring by remember { mutableStateOf(!state.storageReady || state.backupPreview != null) }
    var password by remember { mutableStateOf("") }
    var confirmation by remember { mutableStateOf("") }
    var acknowledged by remember(state.backupPreview) { mutableStateOf(false) }
    val picking = state.backupExportReady
    val locked = state.busy || picking
    val importFile = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) vm.inspectBackupFile(uri, password)
    }
    val saveFile = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri ->
        vm.saveBackup(uri)
    }
    LaunchedEffect(state.backupExportReady) {
        if (state.backupExportReady) { password = ""; confirmation = ""; saveFile.launch("tundra-backup.tundra") }
    }
    DisposableEffect(vm) { onDispose { vm.cancelBackup() } }
    ModalBottomSheet(onDismissRequest = { if (!locked) { password = ""; confirmation = ""; vm.cancelBackup(); onClose() } },
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.padding(24.dp).imePadding().verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Text("Backup and recovery", style = MaterialTheme.typography.headlineSmall)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(!restoring, { restoring = false; password = ""; confirmation = ""; vm.cancelBackup() },
                    label = { Text("Export") }, enabled = !locked && state.storageReady)
                FilterChip(restoring, { restoring = true; password = ""; confirmation = ""; vm.cancelBackup() },
                    label = { Text("Restore") }, enabled = !locked)
            }
            Text(if (restoring) "Restore public wallets and their private metadata from an encrypted Tundra backup. Hardware signing keys are never in this file."
                else "Export all wallets, labels, freezes and saved payment history. The file is encrypted with its own password.")
            Text("Use a strong, unique password of at least 16 characters and keep it separately. Spaces and Unicode are significant; a lost password cannot be recovered.")
            OutlinedTextField(password, { password = it }, label = { Text("Backup password") }, singleLine = true,
                visualTransformation = PasswordVisualTransformation(), keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false),
                enabled = !locked, modifier = Modifier.fillMaxWidth().testTag("backupPassword"))
            if (!restoring) {
                OutlinedTextField(confirmation, { confirmation = it }, label = { Text("Confirm backup password") }, singleLine = true,
                    visualTransformation = PasswordVisualTransformation(), keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false),
                    enabled = !locked, modifier = Modifier.fillMaxWidth().testTag("backupPasswordConfirmation"))
                Button(onClick = { vm.prepareBackup(password) }, enabled = !locked && password.isNotEmpty() && password == confirmation,
                    modifier = Modifier.testTag("prepareBackup")) { Text("Create encrypted backup") }
            } else {
                Button(onClick = { importFile.launch(arrayOf("*/*")) }, enabled = !locked && password.isNotEmpty()) { Text("Choose backup file") }
                state.backupPreview?.let { info ->
                    Text("Review backup", style = MaterialTheme.typography.titleMedium)
                    Text("${info.wallets.size} wallets · ${info.drafts} saved payments · ${info.submissions} submission records")
                    info.wallets.forEach { Text("${it.name} · ${policyText(it.policy)}") }
                    Text("Balances will be unknown until a new sync. Saved approvals are suspended, and unresolved submission inputs remain held.")
                    Text("An old backup cannot know receive addresses issued later. Sync cannot discover unused addresses, so recovery may reuse one. Compare receive addresses and policies on your hardware.")
                    Text("Restore switches to a new protected store. Existing wallet files and their keys are retained on this device.")
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(acknowledged, { acknowledged = it }, enabled = !locked, modifier = Modifier.testTag("backupRestoreConsent"))
                        Text("I understand these recovery limits and want to use this backup")
                    }
                    Button(onClick = {
                        val entered = password; password = ""; confirmation = ""
                        vm.restoreBackup(entered, acknowledged); acknowledged = false
                    }, enabled = !locked && acknowledged && password.isNotEmpty(), modifier = Modifier.testTag("restoreBackup")) { Text("Restore reviewed backup") }
                }
            }
            if (state.busy) CircularProgressIndicator(Modifier.size(24.dp))
            state.backupMessage?.let { Text(it, modifier = Modifier.testTag("backupResult")) }
            state.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            TextButton(onClick = { password = ""; confirmation = ""; vm.cancelBackup(); onClose() }, enabled = !locked) { Text("Close") }
            Spacer(Modifier.height(24.dp))
        }
    }
}

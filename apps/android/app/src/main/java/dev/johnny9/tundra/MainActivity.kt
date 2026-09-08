@file:OptIn(ExperimentalUnsignedTypes::class, androidx.compose.material3.ExperimentalMaterial3Api::class)
package dev.johnny9.tundra

import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Modifier
import androidx.compose.ui.Alignment
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.platform.testTag
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import dev.johnny9.tundra.generated.*

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        setContent {
            val vm: WalletViewModel = viewModel()
            val state by vm.state.collectAsStateWithLifecycle()
            TundraTheme(state.dark) { WalletApp(vm, state) }
        }
    }
}
fun policyText(policy: WalletPolicy) = if (policy == WalletPolicy.SINGLE_SIG) "Single signature" else "2 of 3 signatures"
@Composable private fun WalletApp(vm: WalletViewModel, s: WalletState) {
    var tab by rememberSaveable { mutableIntStateOf(0) }
    var settings by remember { mutableStateOf(false) }
    var add by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<CoinInfo?>(null) }
    var labelText by remember { mutableStateOf("") }
    var syncSheet by remember { mutableStateOf(false) }
    var paymentSheet by remember { mutableStateOf(false) }
    var paymentMode by remember { mutableIntStateOf(0) }
    var query by remember { mutableStateOf("") }
    var largestFirst by remember { mutableStateOf(false) }
    var availableOnly by remember { mutableStateOf(false) }
    var bulkLabel by remember { mutableStateOf(false) }
    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner, vm) {
        val observer = LifecycleEventObserver { _, event -> if (event == Lifecycle.Event.ON_STOP) vm.cancelSync() }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }
    val importLabels = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { it?.let(vm::inspectLabels) }
    val exportLabels = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/jsonl")) { it?.let(vm::exportLabels) }
    Surface(Modifier.fillMaxSize()) {
        Column(Modifier.safeDrawingPadding().padding(horizontal = 24.dp)) {
            Row(Modifier.fillMaxWidth().height(68.dp), verticalAlignment = Alignment.CenterVertically) {
                TundraMark(); Spacer(Modifier.width(10.dp))
                Box(Modifier.weight(1f)) {
                    TextButton(onClick = { menu = true }, enabled = !s.busy) {
                        Text(s.wallet?.name ?: "Tundra", style = MaterialTheme.typography.titleLarge)
                        Icon(Icons.Outlined.ExpandMore, "Choose wallet")
                    }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        s.wallets.forEach { w -> DropdownMenuItem(text = { Text(w.name) }, onClick = { menu = false; vm.select(w.id) }) }
                        DropdownMenuItem(text = { Text("Add wallet") }, onClick = { menu = false; add = true })
                    }
                }
                IconButton(onClick = { settings = true }, enabled = !s.busy) { Icon(Icons.Outlined.Settings, "Settings") }
            }
            Text("Development build · test data only", color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.labelSmall)
            if (s.wallet == null) {
                Spacer(Modifier.height(60.dp))
                Text("Your bitcoin.\nYour hardware.", fontSize = 32.sp, fontWeight = FontWeight.Medium)
                Spacer(Modifier.height(16.dp))
                Text("Import a public wallet descriptor. Your signing keys stay on your hardware.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.height(24.dp))
                Button(onClick = { add = true }, enabled = !s.busy, modifier = Modifier.fillMaxWidth()) { Text("Add wallet") }
            } else {
                Spacer(Modifier.height(20.dp))
                Text(s.wallet!!.totalSats?.let { "${formatBalance(it)} BTC" } ?: "— BTC", fontSize = 30.sp, fontWeight = FontWeight.Medium)
                Text(if (s.wallet!!.synced) "${s.wallet!!.availableSats?.let(::formatBalance) ?: "—"} BTC available" else "Not synced · balance unknown", color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(s.wallet!!.syncedAt?.let { "Cached · last sync ${java.time.Instant.ofEpochSecond(it.toLong())}" } ?: "No successful scan", style = MaterialTheme.typography.bodySmall)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    TextButton(onClick = { syncSheet = true }, enabled = !s.busy) { Text("Sync test network") }
                    s.sync?.let { op ->
                        if (op.state in listOf(SyncState.PREPARED, SyncState.SCANNING, SyncState.APPLYING)) {
                            Text("${op.scannedScripts} scripts", style = MaterialTheme.typography.bodySmall)
                            TextButton(onClick = vm::cancelSync) { Text("Cancel sync") }
                        }
                    }
                }
                Spacer(Modifier.height(20.dp))
                // Shared header stays outside either lazy list; switching tabs cannot move it.
                TabRow(selectedTabIndex = tab) {
                    Tab(selected = tab == 0, onClick = { tab = 0 }, text = { Text("Activity") })
                    Tab(selected = tab == 1, onClick = { tab = 1 }, text = { Text("Coins") })
                }
                if (tab == 0) {
                    Row(Modifier.fillMaxWidth().padding(vertical = 20.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        OutlinedButton(onClick = vm::receive, enabled = !s.busy, modifier = Modifier.weight(1f)) { Text("Receive") }
                        Button(onClick = { vm.closeReview(); paymentMode = 0; paymentSheet = true }, enabled = !s.busy && s.wallet!!.synced, modifier = Modifier.weight(1f)) { Text("Send") }
                    }
                    Text("Hardware signing is not qualified. Use disposable test wallets only.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    LazyColumn(Modifier.weight(1f).padding(top = 16.dp)) {
                        items(s.drafts, key = { "draft-${it.id}" }) { draft ->
                            ListItem(modifier = Modifier.clickable(enabled = !s.busy) { vm.openReview(draft); paymentSheet = true },
                                headlineContent = { Text(draft.label.ifBlank { "Saved payment" }) },
                                supportingContent = { Text("Draft · ${draft.state} · ${draft.inputs.size} inputs") })
                        }
                        if (s.activity.isEmpty()) item { EmptyState("No activity", "Transactions appear after a successful scan of your chosen endpoint.") }
                        items(s.activity, key = { it.txid }) { tx ->
                            ListItem(headlineContent = { Text(tx.label.ifBlank { "Unlabeled transaction" }) }, supportingContent = { Text(if (tx.confirmed) "Confirmed" else "Pending") })
                        }
                    }
                } else {
                    OutlinedTextField(query, { query = it }, label = { Text("Search labels, addresses, outpoints") }, singleLine = true, modifier = Modifier.fillMaxWidth().padding(top = 12.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FilterChip(availableOnly, { availableOnly = !availableOnly }, label = { Text("Available") })
                        FilterChip(largestFirst, { largestFirst = !largestFirst }, label = { Text("Largest first") })
                    }
                    if (s.selected.isNotEmpty()) {
                        Text("${s.selected.size} selected · ${formatBalance(s.coins.filter { it.outpoint in s.selected }.fold(0uL) { total, coin -> total + coin.sats })} BTC")
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            TextButton(onClick = { vm.closeReview(); paymentMode = 0; paymentSheet = true }, enabled = !s.busy) { Text("Send") }
                            TextButton(onClick = { vm.closeReview(); paymentMode = 2; paymentSheet = true }, enabled = !s.busy && s.selected.size >= 2) { Text("Consolidate") }
                            TextButton(onClick = { labelText = ""; bulkLabel = true }, enabled = !s.busy) { Text("Label") }
                        }
                        Row {
                            TextButton(onClick = { vm.bulkEdit(null, true) }, enabled = !s.busy) { Text("Freeze selected") }
                            TextButton(onClick = vm::clearSelection) { Text("Clear selection") }
                        }
                    }
                    val filtered = s.coins.filter { coin ->
                        (!availableOnly || coin.state == CoinState.AVAILABLE) &&
                            listOf(coin.label, coin.address, coin.outpoint).any { it.contains(query, ignoreCase = true) }
                    }.let { coins -> if (largestFirst) coins.sortedByDescending { it.sats } else coins.sortedBy { it.label.lowercase() } }
                    LazyColumn(Modifier.weight(1f).padding(top = 16.dp)) {
                        if (s.coins.isEmpty()) item { EmptyState("No synced coins", "No sample UTXOs are injected. Labels and coin control operate on wallet-owned outputs.") }
                        items(filtered, key = { it.outpoint }) { coin ->
                            ListItem(modifier = Modifier.clickable(enabled = !s.busy) { editing = coin; labelText = coin.label },
                                headlineContent = { Text(coin.label.ifBlank { "Add a label" }) },
                                leadingContent = { Checkbox(checked = coin.outpoint in s.selected, onCheckedChange = { vm.toggleCoin(coin) }, enabled = !s.busy && coin.state == CoinState.AVAILABLE,
                                    modifier = Modifier.semantics { contentDescription = "Select coin, ${formatBalance(coin.sats)} BTC" }) },
                                supportingContent = { Text(coin.state.name.lowercase().replace('_', ' ').replaceFirstChar { it.uppercase() }) },
                                trailingContent = { Text("${formatBalance(coin.sats)} BTC") })
                        }
                    }
                }
            }
            if (s.busy) LinearProgressIndicator(Modifier.fillMaxWidth())
        }
    }
    if (add) ImportSheet(vm, s, onClose = { vm.cancelImport(); add = false })
    if (syncSheet) SyncSheet(s.endpoint, onClose = { syncSheet = false }, onSync = { endpoint, consent -> syncSheet = false; vm.sync(endpoint, consent) })
    if (paymentSheet) PaymentSheet(vm, s, paymentMode, onClose = { vm.closeReview(); paymentSheet = false })
    if (bulkLabel) AlertDialog(onDismissRequest = { bulkLabel = false }, title = { Text("Label selected coins") },
        text = { OutlinedTextField(labelText, { labelText = it }, label = { Text("Label") }) },
        confirmButton = { TextButton(onClick = { vm.bulkEdit(labelText, null); bulkLabel = false }) { Text("Apply") } },
        dismissButton = { TextButton(onClick = { bulkLabel = false }) { Text("Cancel") } })
    if (settings) ModalBottomSheet(onDismissRequest = { settings = false }) {
        Column(Modifier.padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Text("Tundra", style = MaterialTheme.typography.headlineSmall)
            Text("Your bitcoin. Your hardware.", color = MaterialTheme.colorScheme.onSurfaceVariant)
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Text("Dark appearance", Modifier.weight(1f)); Switch(checked = s.dark, onCheckedChange = vm::appearance)
            }
            Text("Labels", style = MaterialTheme.typography.titleMedium)
            Text("Exports are unencrypted and privacy-sensitive. Only known references are matched; origin-tagged records must match this wallet's policy and key origins.", style = MaterialTheme.typography.bodySmall)
            OutlinedButton(onClick = { importLabels.launch(arrayOf("*/*")) }, enabled = s.wallet != null && !s.busy) { Text("Import labels") }
            OutlinedButton(onClick = { exportLabels.launch("tundra-labels.jsonl") }, enabled = s.wallet != null && !s.busy) { Text("Export labels") }
            Text("v0.1.0-dev.1 · Test-network development", style = MaterialTheme.typography.labelSmall)
            Spacer(Modifier.height(24.dp))
        }
    }
    if (s.receive != null) AlertDialog(onDismissRequest = vm::closeReceive,
        title = { Text("Unverified test address") }, text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(s.receive.address)
                Text("Index ${s.receive.index}. Hardware verification is not available. Do not fund this address.")
            }
        }, confirmButton = { TextButton(onClick = vm::closeReceive) { Text("Close") } })
    editing?.let { coin ->
        AlertDialog(onDismissRequest = { editing = null }, title = { Text("Coin label") }, text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(value = labelText, onValueChange = { labelText = it }, label = { Text("Label") }, singleLine = true)
                Text(coin.outpoint, style = MaterialTheme.typography.bodySmall)
                TextButton(onClick = { vm.freeze(coin, coin.state != CoinState.FROZEN); editing = null }) { Text(if (coin.state == CoinState.FROZEN) "Unfreeze coin" else "Freeze coin") }
            }
        }, confirmButton = { TextButton(onClick = { vm.label("output", coin.outpoint, labelText); editing = null }) { Text("Save") } }, dismissButton = { TextButton(onClick = { editing = null }) { Text("Cancel") } })
    }
    s.labelPreview?.let { p -> AlertDialog(onDismissRequest = vm::cancelLabels, title = { Text("Import labels?") },
        text = { Text("${p.matched} matched · ${p.changed} changed · ${p.skipped} skipped. Freezes may change, but draft reservations will not.") },
        confirmButton = { TextButton(onClick = vm::applyLabels, enabled = !s.busy) { Text("Apply") } },
        dismissButton = { TextButton(onClick = vm::cancelLabels) { Text("Cancel") } }) }
    s.error?.let { message -> AlertDialog(onDismissRequest = vm::clearError, title = { Text("Could not complete") }, text = { Text(message) }, confirmButton = { TextButton(onClick = vm::clearError) { Text("OK") } }) }
}
@Composable private fun SyncSheet(savedEndpoint: String, onClose: () -> Unit, onSync: (String, Boolean) -> Unit) {
    var endpoint by remember { mutableStateOf(savedEndpoint) }
    var consent by remember { mutableStateOf(false) }
    ModalBottomSheet(onDismissRequest = onClose, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.padding(24.dp).verticalScroll(rememberScrollState()).imePadding(), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Text("Connect a test endpoint", style = MaterialTheme.typography.headlineSmall)
            OutlinedTextField(value = endpoint, onValueChange = { if (it.length <= 2048) endpoint = it }, label = { Text("Esplora API URL") }, singleLine = true, modifier = Modifier.fillMaxWidth())
            Text("Your server can associate requested script hashes and transactions with your IP address. Descriptors and labels stay on this device. The server supplies your view of the test chain.")
            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(checked = consent, onCheckedChange = { consent = it })
                Text("I trust this endpoint and agree to these requests")
            }
            Button(onClick = { onSync(endpoint, consent) }, enabled = consent && endpoint.isNotBlank()) { Text("Start scan") }
            TextButton(onClick = onClose) { Text("Cancel") }
        }
    }
}
@Composable private fun EmptyState(title: String, detail: String) {
    Column(Modifier.fillMaxWidth().padding(vertical = 24.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text(title, style = MaterialTheme.typography.titleMedium)
        Text(detail, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}
@Composable private fun ImportSheet(vm: WalletViewModel, s: WalletState, onClose: () -> Unit) {
    var scanning by remember { mutableStateOf(false) }
    var payload by remember { mutableStateOf("") }
    var name by remember { mutableStateOf("Savings") }
    // Test-network only native first milestone. The core can inspect mainnet public descriptors.
    var chain by remember { mutableStateOf(Chain.SIGNET) }
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { it?.let { uri -> vm.inspectFile(uri, chain) } }
    ModalBottomSheet(onDismissRequest = { if (!s.busy) onClose() }, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.padding(24.dp).verticalScroll(rememberScrollState()).imePadding(), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Text("Add wallet", style = MaterialTheme.typography.headlineSmall)
            Text("Public descriptors only. Never enter a seed or private key.", color = MaterialTheme.colorScheme.onSurfaceVariant)
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                FilterChip(selected = chain == Chain.SIGNET, onClick = { chain = Chain.SIGNET; vm.cancelImport() }, label = { Text("Signet") }, enabled = !s.busy)
                FilterChip(selected = chain == Chain.REGTEST, onClick = { chain = Chain.REGTEST; vm.cancelImport() }, label = { Text("Regtest") }, enabled = !s.busy)
            }
            if (s.importPreview == null) {
                OutlinedButton(onClick = { picker.launch(arrayOf("*/*")) }, enabled = !s.busy, modifier = Modifier.fillMaxWidth()) { Text("Import descriptor file") }
                OutlinedButton(onClick = { scanning = true }, enabled = !s.busy, modifier = Modifier.fillMaxWidth()) { Text("Scan descriptor QR") }
                OutlinedTextField(value = payload, onValueChange = { if (it.length <= 32768) payload = it }, label = { Text("Or paste a public descriptor") }, modifier = Modifier.fillMaxWidth().heightIn(min = 120.dp, max = 200.dp), enabled = !s.busy)
                Button(onClick = { vm.inspect(payload, chain) }, enabled = payload.isNotBlank() && !s.busy, modifier = Modifier.fillMaxWidth()) { Text("Review wallet") }
                Text("Public descriptor text, UR bytes or BBQr text/JSON. Device-specific account QR formats are not yet supported.", style = MaterialTheme.typography.bodySmall)
            } else {
                Text(policyText(s.importPreview.policy), style = MaterialTheme.typography.titleMedium)
                Text(s.importPreview.firstAddress, style = MaterialTheme.typography.bodySmall)
                OutlinedTextField(value = name, onValueChange = { name = it }, label = { Text("Wallet name") }, singleLine = true)
                Button(onClick = { vm.importWallet(name) }, enabled = name.isNotBlank() && !s.busy, modifier = Modifier.fillMaxWidth().testTag("confirmImport")) { Text("Add wallet") }
                TextButton(onClick = vm::cancelImport, enabled = !s.busy) { Text("Use a different descriptor") }
            }
            TextButton(onClick = onClose, enabled = !s.busy, modifier = Modifier.testTag("closeImport")) { Text("Close") }
            Spacer(Modifier.height(16.dp))
        }
    }
    if (scanning) QrScanDialog(QrPurpose.Descriptor(chain), onPayload = { bytes ->
        scanning = false; vm.inspect(bytes.toString(Charsets.UTF_8), chain)
    }, onClose = { scanning = false })
}

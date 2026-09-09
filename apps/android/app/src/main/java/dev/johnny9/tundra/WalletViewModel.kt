@file:OptIn(ExperimentalUnsignedTypes::class)
package dev.johnny9.tundra

import android.app.Application
import android.net.Uri
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import dev.johnny9.tundra.generated.*
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext

/** Presentation state only. Wallet decisions and persistence remain in Rust. */
data class WalletState(
    val wallets: List<WalletInfo> = emptyList(), val selectedId: String? = null,
    val coins: List<CoinInfo> = emptyList(), val activity: List<ActivityInfo> = emptyList(),
    val busy: Boolean = false, val error: String? = null, val dark: Boolean = true,
    val storageReady: Boolean = false,
    val importPreview: WalletPreview? = null, val receive: AddressInfo? = null,
    val labelPreview: LabelImportPreview? = null,
    val endpoint: String = "", val sync: SyncInfo? = null,
    val selected: Set<String> = emptySet(), val drafts: List<PaymentReview> = emptyList(),
    val review: PaymentReview? = null, val signing: SigningInfo? = null,
    val outputSource: PaymentReview? = null, val sourceOutpoint: String? = null,
    val finalized: FinalTransactionInfo? = null, val submission: BroadcastInfo? = null, val qrFrames: List<String>? = null
) { val wallet: WalletInfo? get() = wallets.firstOrNull { it.id == selectedId } }

class WalletViewModel(application: Application) : AndroidViewModel(application) {
    private val preferences = application.getSharedPreferences("appearance", 0)
    private val mutable = MutableStateFlow(WalletState(dark = preferences.getBoolean("dark", true)))
    val state = mutable.asStateFlow()
    private var core: Tundra? = null
    private var pendingDescriptor: String? = null
    private var pendingChain = Chain.SIGNET
    private var pendingLabels: String? = null
    private var pendingLabelsWallet: String? = null
    init { openStorage() }
    fun openStorage() = run {
        if (mutable.value.storageReady) return@run
        val previous = core; core = null
        core = withContext(Dispatchers.IO) { previous?.close(); StorageVault.open(getApplication<Application>()) }
        refresh()
        mutable.value = mutable.value.copy(storageReady = true)
    }
    // The USB adapter calls this on its IO coroutine and owns the returned native handle.
    fun prepareUsb(walletId: String, operation: UsbOperation): UsbConnection {
        check(mutable.value.selectedId == walletId)
        return engine().prepareUsb(walletId, operation)
    }
    private fun engine() = checkNotNull(core) { "Wallet core is not ready" }
    private fun run(block: suspend () -> Unit) {
        if (mutable.value.busy) return
        viewModelScope.launch {
            mutable.value = mutable.value.copy(busy = true, error = null)
            try { block() }
            catch (e: CancellationException) { throw e }
            catch (e: Exception) {
                // Do not log descriptors, labels, addresses, database paths or native stack traces.
                mutable.value = mutable.value.copy(error = when (e) {
                    is StorageAccessException -> e.message
                    is AppException.Operation -> e.detail
                    else -> "Operation failed. Please try again."
                })
            } finally { mutable.value = mutable.value.copy(busy = false) }
        }
    }
    private suspend fun refresh() {
        val wallets = withContext(Dispatchers.IO) { engine().wallets() }
        val id = mutable.value.selectedId?.takeIf { id -> wallets.any { it.id == id } } ?: wallets.firstOrNull()?.id
        val coins = if (id == null) emptyList() else withContext(Dispatchers.IO) { engine().coins(id) }
        val activity = if (id == null) emptyList() else withContext(Dispatchers.IO) { engine().activity(id) }
        val endpoint = if (id == null) "" else withContext(Dispatchers.IO) { engine().syncEndpoint(id) ?: "" }
        val drafts = if (id == null) emptyList() else withContext(Dispatchers.IO) { engine().drafts(id) }
        val selected = mutable.value.selected.intersect(coins.filter { it.state == CoinState.AVAILABLE }.map { it.outpoint }.toSet())
        val review = mutable.value.review?.let { old -> drafts.firstOrNull { it.id == old.id } }
        mutable.value = mutable.value.copy(wallets = wallets, selectedId = id, coins = coins, activity = activity, endpoint = endpoint, drafts = drafts, selected = selected, review = review, signing = null, finalized = null, submission = null)
        val submission = review?.let { withContext(Dispatchers.IO) { engine().broadcastStatus(it.walletId, it.id) } }
        mutable.value = mutable.value.copy(submission = submission)
        if (review != null && review.state !in listOf("invalidated", "observed") && submission == null) {
            val signing = withContext(Dispatchers.IO) { engine().signingProgress(review.walletId, review.id) }
            val finalized = withContext(Dispatchers.IO) { engine().finalizedDraft(review.walletId, review.id) }
            if (mutable.value.review?.id == review.id) mutable.value = mutable.value.copy(signing = signing, finalized = finalized)
        }
    }
    fun select(id: String) = run { mutable.value = mutable.value.copy(selectedId = id, receive = null, selected = emptySet(), review = null, qrFrames = null); refresh() }
    fun appearance(dark: Boolean) { preferences.edit().putBoolean("dark", dark).apply(); mutable.value = mutable.value.copy(dark = dark) }
    fun clearError() { mutable.value = mutable.value.copy(error = null) }
    fun cancelImport() { pendingDescriptor = null; mutable.value = mutable.value.copy(importPreview = null) }
    fun inspect(payload: String, chain: Chain) = run {
        pendingDescriptor = null
        mutable.value = mutable.value.copy(importPreview = null)
        val preview = withContext(Dispatchers.IO) { engine().previewImport(payload, chain) }
        pendingDescriptor = payload; pendingChain = chain
        mutable.value = mutable.value.copy(importPreview = preview)
    }
    fun inspectFile(uri: Uri, chain: Chain) = run {
        pendingDescriptor = null
        mutable.value = mutable.value.copy(importPreview = null)
        val payload = readBounded(uri, 32_768)
        val preview = withContext(Dispatchers.IO) { engine().previewImport(payload, chain) }
        pendingDescriptor = payload; pendingChain = chain
        mutable.value = mutable.value.copy(importPreview = preview)
    }
    fun importWallet(name: String) = run {
        val payload = checkNotNull(pendingDescriptor)
        val w = withContext(Dispatchers.IO) { engine().importWallet(name, payload, pendingChain) }
        cancelImport(); mutable.value = mutable.value.copy(selectedId = w.id); refresh()
    }
    fun receive() = run {
        val id = checkNotNull(mutable.value.selectedId)
        val address = withContext(Dispatchers.IO) { engine().receiveAddress(id) }
        mutable.value = mutable.value.copy(receive = address)
    }
    fun closeReceive() { mutable.value = mutable.value.copy(receive = null) }
    fun toggleCoin(coin: CoinInfo) {
        if (mutable.value.busy || coin.state != CoinState.AVAILABLE) return
        val selected = mutable.value.selected.toMutableSet()
        if (!selected.remove(coin.outpoint)) selected.add(coin.outpoint)
        mutable.value = mutable.value.copy(selected = selected)
    }
    fun clearSelection() { mutable.value = mutable.value.copy(selected = emptySet()) }
    fun loadOutputSource(outpoint: String) = run {
        val walletId = checkNotNull(mutable.value.selectedId)
        mutable.value = mutable.value.copy(outputSource = null, sourceOutpoint = outpoint)
        val source = withContext(Dispatchers.IO) { engine().outputSource(walletId, outpoint) }
        if (mutable.value.selectedId == walletId && mutable.value.sourceOutpoint == outpoint) {
            mutable.value = mutable.value.copy(outputSource = source)
        }
    }
    fun bulkEdit(label: String?, frozen: Boolean?) = run {
        val s = mutable.value
        withContext(Dispatchers.IO) { engine().editCoins(checkNotNull(s.selectedId), s.selected.toList(), label, frozen) }
        refresh()
    }
    fun openReview(review: PaymentReview) { mutable.value = mutable.value.copy(review = review, signing = null, finalized = null, submission = null); run { refresh() } }
    fun closeReview() { mutable.value = mutable.value.copy(review = null, signing = null, finalized = null, submission = null, qrFrames = null) }
    fun createPayment(mode: Int, address: String, amount: String, fee: String, label: String, automatic: Boolean, acknowledge: Boolean) = run {
        val s = mutable.value
        val review = withContext(Dispatchers.IO) {
            val intent = when (mode) {
                1 -> PaymentIntent.SendMax(address)
                2 -> PaymentIntent.Consolidate(acknowledge)
                else -> PaymentIntent.Send(address, parseBtcAmount(amount))
            }
            engine().createDraft(PaymentRequest(checkNotNull(s.selectedId), intent,
                if (automatic && mode == 0) null else s.selected.toList(), parseFeeRate(fee), label))
        }
        mutable.value = mutable.value.copy(review = review, selected = emptySet())
        refresh()
    }
    fun discardReview() = run {
        val review = checkNotNull(mutable.value.review)
        withContext(Dispatchers.IO) { engine().discardDraft(review.walletId, review.id) }
        mutable.value = mutable.value.copy(review = null); refresh()
    }
    fun exportDraft(uri: Uri, walletId: String, draftId: String) = run {
        withContext(Dispatchers.IO) {
            val payload = engine().exportSigningPsbt(walletId, draftId)
            getApplication<Application>().contentResolver.openOutputStream(uri, "wt")?.use {
                it.write(payload.toByteArray(Charsets.US_ASCII)); it.flush()
            } ?: error("Could not open output")
        }
    }
    fun importSignedDraft(uri: Uri, walletId: String, draftId: String) = run {
        require(mutable.value.selectedId == walletId && mutable.value.review?.id == draftId) { "Review changed" }
        val payload = readBytesBounded(uri, 1_398_106)
        withContext(Dispatchers.IO) { engine().acceptSignedPsbt(walletId, draftId, payload) }
        refresh()
    }
    fun importSignedQr(payload: ByteArray, walletId: String, draftId: String) = run {
        require(mutable.value.selectedId == walletId && mutable.value.review?.id == draftId) { "Review changed" }
        withContext(Dispatchers.IO) { engine().acceptSignedPsbt(walletId, draftId, payload) }
        refresh()
    }
    fun exportQr(encoding: QrEncoding) = run {
        val review = checkNotNull(mutable.value.review)
        val frames = withContext(Dispatchers.IO) { engine().exportDraftQr(review.walletId, review.id, encoding) }
        mutable.value = mutable.value.copy(qrFrames = frames)
    }
    fun finalizeReview() = run {
        val review = mutable.value.review ?: return@run
        withContext(Dispatchers.IO) { engine().finalizeDraft(review.walletId, review.id) }
        refresh()
    }
    fun submitBroadcast(request: BroadcastRequest) = run {
        check(mutable.value.review?.id == request.draftId && mutable.value.selectedId == request.walletId)
        try { withContext(Dispatchers.IO) { engine().broadcastDraft(request) } }
        finally { refresh() } // Re-read durable uncertainty even if acknowledgement persistence failed.
    }
    fun closeQr() { mutable.value = mutable.value.copy(qrFrames = null) }
    fun sync(endpoint: String, consent: Boolean) = run {
        val id = checkNotNull(mutable.value.selectedId)
        val operation = withContext(Dispatchers.IO) { engine().prepareSync(id, endpoint, consent) }
        mutable.value = mutable.value.copy(sync = operation)
        try {
            withContext(Dispatchers.IO) { engine().runSync(operation.id) }
            while (true) {
                val progress = withContext(Dispatchers.IO) { engine().syncProgress(operation.id) }
                mutable.value = mutable.value.copy(sync = progress)
                if (progress.state in listOf(SyncState.COMPLETE, SyncState.CANCELLED, SyncState.FAILED)) {
                    mutable.value = mutable.value.copy(error = progress.error)
                    refresh(); break
                }
                delay(150)
            }
        } finally {
            // Idempotent; if commit won, the core retains its completed result.
            withContext(kotlinx.coroutines.NonCancellable + Dispatchers.IO) { engine().cancelSync(operation.id) }
        }
    }
    fun cancelSync() {
        val id = mutable.value.sync?.id ?: return
        viewModelScope.launch { withContext(Dispatchers.IO) { core?.cancelSync(id) } }
    }
    override fun onCleared() {
        mutable.value.sync?.id?.let { core?.cancelSync(it) }
        super.onCleared()
    }
    fun label(kind: String, reference: String, value: String) = run {
        val id = checkNotNull(mutable.value.selectedId)
        withContext(Dispatchers.IO) { engine().setLabel(id, kind, reference, value) }; refresh()
    }
    fun freeze(coin: CoinInfo, frozen: Boolean) = run {
        val id = checkNotNull(mutable.value.selectedId)
        withContext(Dispatchers.IO) { engine().setFrozen(id, coin.outpoint, frozen) }; refresh()
    }
    fun inspectLabels(uri: Uri) = run {
        val id = checkNotNull(mutable.value.selectedId)
        val payload = readBounded(uri, 2 * 1024 * 1024)
        val preview = withContext(Dispatchers.IO) { engine().importLabels(id, payload, false) }
        pendingLabels = payload; pendingLabelsWallet = id; mutable.value = mutable.value.copy(labelPreview = preview)
    }
    fun applyLabels() = run {
        val id = checkNotNull(pendingLabelsWallet); val payload = checkNotNull(pendingLabels)
        require(id == mutable.value.selectedId) { "Wallet changed; preview labels again" }
        withContext(Dispatchers.IO) { engine().importLabels(id, payload, true) }
        pendingLabels = null; pendingLabelsWallet = null; mutable.value = mutable.value.copy(labelPreview = null); refresh()
    }
    fun cancelLabels() { pendingLabels = null; pendingLabelsWallet = null; mutable.value = mutable.value.copy(labelPreview = null) }
    fun exportLabels(uri: Uri) = run {
        val id = checkNotNull(mutable.value.selectedId)
        withContext(Dispatchers.IO) {
            val payload = engine().exportLabels(id)
            getApplication<Application>().contentResolver.openOutputStream(uri, "wt")?.use {
                it.write(payload.toByteArray(Charsets.UTF_8)); it.flush()
            } ?: error("Could not open output")
        }
    }
    private suspend fun readBounded(uri: Uri, max: Int): String = withContext(Dispatchers.IO) {
        Charsets.UTF_8.newDecoder().decode(java.nio.ByteBuffer.wrap(readBytesBounded(uri, max))).toString()
    }
    private suspend fun readBytesBounded(uri: Uri, max: Int): ByteArray = withContext(Dispatchers.IO) {
        getApplication<Application>().contentResolver.openInputStream(uri)?.use {
            val output = java.io.ByteArrayOutputStream()
            val buffer = ByteArray(8192)
            while (true) {
                val count = it.read(buffer, 0, minOf(buffer.size, max + 1 - output.size()))
                if (count < 0) break
                if (count == 0) continue
                output.write(buffer, 0, count)
                require(output.size() <= max) { "File too large" }
            }
            output.toByteArray()
        } ?: error("Could not open input")
    }
}

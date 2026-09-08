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
import kotlinx.coroutines.withContext

/** Presentation state only. Wallet decisions and persistence remain in Rust. */
data class WalletState(
    val wallets: List<WalletInfo> = emptyList(), val selectedId: String? = null,
    val coins: List<CoinInfo> = emptyList(), val activity: List<ActivityInfo> = emptyList(),
    val busy: Boolean = false, val error: String? = null, val dark: Boolean = true,
    val importPreview: WalletPreview? = null, val receive: AddressInfo? = null,
    val labelPreview: LabelImportPreview? = null
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
    init { run { core = withContext(Dispatchers.IO) { Tundra.open(application.noBackupFilesDir.resolve("tundra.sqlite").path) }; refresh() } }
    private fun engine() = checkNotNull(core) { "Wallet core is not ready" }
    private fun run(block: suspend () -> Unit) {
        if (mutable.value.busy) return
        viewModelScope.launch {
            mutable.value = mutable.value.copy(busy = true, error = null)
            try { block() }
            catch (e: CancellationException) { throw e }
            catch (e: Exception) {
                // Do not log descriptors, labels, addresses, database paths or native stack traces.
                mutable.value = mutable.value.copy(error = if (e is AppException) e.message else "Operation failed. Please try again.")
            } finally { mutable.value = mutable.value.copy(busy = false) }
        }
    }
    private suspend fun refresh() {
        val wallets = withContext(Dispatchers.IO) { engine().wallets() }
        val id = mutable.value.selectedId?.takeIf { id -> wallets.any { it.id == id } } ?: wallets.firstOrNull()?.id
        val coins = if (id == null) emptyList() else withContext(Dispatchers.IO) { engine().coins(id) }
        val activity = if (id == null) emptyList() else withContext(Dispatchers.IO) { engine().activity(id) }
        mutable.value = mutable.value.copy(wallets = wallets, selectedId = id, coins = coins, activity = activity)
    }
    fun select(id: String) = run { mutable.value = mutable.value.copy(selectedId = id, receive = null); refresh() }
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
            val bytes = output.toByteArray()
            Charsets.UTF_8.newDecoder().decode(java.nio.ByteBuffer.wrap(bytes)).toString()
        } ?: error("Could not open input")
    }
}

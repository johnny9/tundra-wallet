package dev.johnny9.tundra

import android.app.Application
import android.net.Uri
import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelStore
import androidx.test.core.app.ApplicationProvider
import androidx.test.platform.app.InstrumentationRegistry
import dev.johnny9.tundra.generated.*
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import java.io.File
import java.security.KeyStore
import java.util.UUID

/** Public fixture only. Exercises native file reading and payment UI; no network calls. */
class SigningRuntimeTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    @Test fun publishedFileResponseUpdatesEachInputAndFinalizationControlUsesExactBytes() {
        val application = ApplicationProvider.getApplicationContext<Application>()
        val assets = InstrumentationRegistry.getInstrumentation().context.assets
        val fixture = JSONObject(assets.open("native-signing.json").bufferedReader().use { it.readText() })
        val wallet = fixture.getString("wallet_id")
        val draft = fixture.getString("draft_id")
        val transaction = fixture.getJSONObject("final")
        val id = UUID.randomUUID().toString()
        val root = File(application.cacheDir, "signing-ui-$id").also { check(it.mkdir()) }
        val alias = "tundra.test.signing.$id"
        val keys = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val views = ViewModelStore()
        try {
            assets.open("native-unsigned.sqlite").use { input ->
                File(root, "tundra.sqlite").outputStream().use { input.copyTo(it) }
            }
            lateinit var vm: WalletViewModel
            compose.runOnUiThread {
                val factory = object : ViewModelProvider.Factory {
                    @Suppress("UNCHECKED_CAST")
                    override fun <T : ViewModel> create(modelClass: Class<T>): T = WalletViewModel(application, root, alias) as T
                }
                vm = ViewModelProvider(views, factory)[WalletViewModel::class.java]
            }
            compose.waitUntil(10_000) { vm.state.value.storageReady && !vm.state.value.busy }
            compose.runOnUiThread { vm.openReview(vm.state.value.drafts.single()) }
            compose.waitUntil(10_000) { !vm.state.value.busy && vm.state.value.signing != null }
            compose.setContent {
                val state by vm.state.collectAsState()
                TundraTheme(true) { PaymentSheet(vm, state, 0, vm::closeReview) }
            }
            compose.onNodeWithText("Network: Signet").assertIsDisplayed()
            for (index in 1..2) compose.onNodeWithText("Input $index: 0 / 1").performScrollTo().assertIsDisplayed()
            compose.onNodeWithText("Finalize for review").assertDoesNotExist()
            val response = File(root, "public-response.psbt")
            assets.open("native-signed-response.psbt").use { input -> response.outputStream().use { input.copyTo(it) } }
            // Invoke the same native reader used by the file-picker callback. The
            // system PSBT picker itself is a separate transport acceptance check.
            compose.runOnUiThread { vm.importSignedDraft(Uri.fromFile(response), wallet, draft) }
            compose.waitUntil(10_000) { vm.state.value.let { !it.busy && (it.signing?.complete == true || it.error != null) } }
            assertNull("Public signing response failed", vm.state.value.error)
            for (index in 1..2) compose.onNodeWithText("Input $index: 1 / 1").performScrollTo().assertIsDisplayed()
            compose.onNodeWithText("Finalize for review").performScrollTo().assertIsDisplayed().assertIsEnabled().performClick()
            compose.waitUntil(10_000) { vm.state.value.let { !it.busy && (it.finalized != null || it.error != null) } }
            assertNull("Public finalization failed", vm.state.value.error)
            val finalized = checkNotNull(vm.state.value.finalized)
            assertEquals(transaction.getString("txid"), finalized.txid)
            assertEquals(transaction.getString("wtxid"), finalized.wtxid)
            assertEquals(transaction.getString("raw"), finalized.transactionBytes.joinToString("") { "%02x".format(it) })
            compose.onNodeWithText("Final transaction").performScrollTo().assertIsDisplayed()
            compose.onNodeWithText(finalized.txid).performScrollTo().assertIsDisplayed()
            compose.onNodeWithText("${finalized.vsize} vB · ${finalized.feeSats} sats fee").performScrollTo().assertIsDisplayed()
            assertEquals("finalized", vm.state.value.review?.state)
            assertNull(vm.state.value.submission)
            StorageVault.openActive(application, root, alias).use { core ->
                assertArrayEquals(finalized.transactionBytes, core.finalizedDraft(wallet, draft)!!.transactionBytes)
                assertNull(core.broadcastStatus(wallet, draft))
            }
        } catch (failure: Throwable) {
            capturePublicFixtureScreenshot("published-signing-screen")
            throw failure
        } finally {
            compose.runOnUiThread { views.clear() }
            keys.deleteEntry(alias); root.deleteRecursively()
        }
    }
}

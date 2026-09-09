package dev.johnny9.tundra

import android.app.Application
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
import java.io.ByteArrayOutputStream
import java.net.InetSocketAddress
import java.net.Socket
import java.security.KeyStore
import java.util.UUID

/** Published signatures and a separate synthetic HTTP fixture, never a real broadcast. */
class RecoveryRuntimeTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()
    private val endpoint = "http://127.0.0.1:3004"

    private fun posts(): Int {
        // Test diagnostics only: a fixed loopback request with a 4 KiB bound.
        // Keep the app's Java cleartext-HTTP policy unchanged. Wallet networking
        // still runs through the consent-gated Rust API, as it does in production.
        return Socket().use { socket ->
            socket.connect(InetSocketAddress("127.0.0.1", 3004), 3_000); socket.soTimeout = 3_000
            socket.getOutputStream().write("GET /_fixture_state HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n".toByteArray(Charsets.US_ASCII))
            val response = ByteArrayOutputStream()
            val buffer = ByteArray(512)
            while (true) {
                val count = socket.getInputStream().read(buffer)
                if (count < 0) break
                check(response.size() + count <= 4096) { "Public fixture status exceeded its bound" }
                response.write(buffer, 0, count)
            }
            val text = response.toString(Charsets.US_ASCII.name())
            check(text.startsWith("HTTP/1.0 200 ")) { "Public fixture status request failed" }
            val separator = text.indexOf("\r\n\r\n")
            check(separator >= 0)
            JSONObject(text.substring(separator + 4)).getInt("posts")
        }
    }

    @Test fun recoveredReviewRequiresSyncAndSeparateSubmissionConsentThenShowsInputProvenance() {
        val application = ApplicationProvider.getApplicationContext<Application>()
        val assets = InstrumentationRegistry.getInstrumentation().context.assets
        val fixture = JSONObject(assets.open("native-signing.json").bufferedReader().use { it.readText() })
        val wallet = fixture.getString("wallet_id")
        val draft = fixture.getString("draft_id")
        val txid = fixture.getJSONObject("final").getString("txid")
        val id = UUID.randomUUID().toString()
        val root = File(application.cacheDir, "recovery-ui-$id").also { check(it.mkdir()) }
        val alias = "tundra.test.recovery.$id"
        val keys = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val views = ViewModelStore()
        try {
            assertEquals("Use a fresh, isolated public screen fixture", 0, posts())
            val backup = File(root, "public-signed.tundra")
            assets.open("native-signed-backup.tundra").use { input -> backup.outputStream().use { input.copyTo(it) } }
            // Restore is setup here. BackupDocumentRuntimeTest separately exercises
            // the real provider picker, password, consent and generation switch.
            StorageVault.restoreActive(application, backup, fixture.getString("password"), root, alias).use {
                assertTrue(it.recoveryRequired(wallet, draft)); assertNull(it.wallets().single().totalSats)
            }
            lateinit var vm: WalletViewModel
            compose.runOnUiThread {
                val factory = object : ViewModelProvider.Factory {
                    @Suppress("UNCHECKED_CAST")
                    override fun <T : ViewModel> create(modelClass: Class<T>): T = WalletViewModel(application, root, alias) as T
                }
                vm = ViewModelProvider(views, factory)[WalletViewModel::class.java]
            }
            fun idle() {
                compose.waitUntil(30_000) { !vm.state.value.busy }
                assertNull("Public recovery operation failed", vm.state.value.error)
            }
            fun sync() {
                compose.runOnUiThread { vm.sync(endpoint, true) }
                compose.waitUntil(30_000) { vm.state.value.let { !it.busy && (it.sync?.state == SyncState.COMPLETE || it.error != null) } }
                assertNull("Public recovery sync failed", vm.state.value.error)
                assertEquals(SyncState.COMPLETE, vm.state.value.sync?.state)
            }
            compose.waitUntil(10_000) { vm.state.value.storageReady && !vm.state.value.busy }
            compose.setContent {
                val state by vm.state.collectAsState()
                TundraTheme(true) { WalletApp(vm, state) }
            }
            compose.onNodeWithText("— BTC").assertIsDisplayed()
            compose.onNodeWithText("Draft · invalidated · 2 inputs").performScrollTo().performClick()
            idle()
            compose.onNodeWithTag("resumeRecovery").performScrollTo().assertIsNotEnabled()
            compose.onNodeWithTag("recoveryConsent").performScrollTo().assertIsNotEnabled()
            assertEquals(0, posts())
            sync()
            assertEquals(2, vm.state.value.coins.count { it.state == CoinState.RESERVED })
            compose.onNodeWithTag("resumeRecovery").performScrollTo().assertIsNotEnabled()
            compose.onNodeWithTag("recoveryConsent").performScrollTo().assertIsEnabled().performClick()
            compose.onNodeWithTag("resumeRecovery").performScrollTo().assertIsEnabled().performClick()
            idle()
            assertFalse(vm.state.value.recoveryRequired)
            assertEquals("finalized", vm.state.value.review?.state)
            assertEquals(txid, vm.state.value.finalized?.txid)
            assertFalse(checkNotNull(vm.state.value.submission).acknowledged)
            assertEquals(1uL, vm.state.value.submission?.attemptId)
            assertEquals(0, posts()) // Resuming the exact approval must not transmit.
            compose.onNodeWithTag("reviewBroadcast").performScrollTo().performClick()
            compose.onNodeWithTag("confirmBroadcast").assertIsNotEnabled()
            compose.onNodeWithTag("broadcastEndpoint").performScrollTo().performTextReplacement(endpoint)
            compose.onNodeWithTag("broadcastEndpoint").performImeAction()
            compose.onNodeWithTag("broadcastConsent").performScrollTo().performClick()
            compose.onNodeWithTag("confirmBroadcast").assertIsNotEnabled()
            compose.onNodeWithTag("broadcastRetryConsent").performScrollTo().performClick()
            compose.onNodeWithTag("confirmBroadcast").assertIsEnabled().performClick()
            idle()
            assertTrue(checkNotNull(vm.state.value.submission).acknowledged)
            assertEquals(2uL, vm.state.value.submission?.attemptId)
            assertEquals(1, posts())
            compose.onNodeWithText("Save for later").performScrollTo().performClick()
            sync()
            assertEquals("observed", vm.state.value.drafts.single().state)
            assertFalse(vm.state.value.coins.any { it.state == CoinState.RESERVED })
            val output = vm.state.value.coins.single { it.outpoint.startsWith("$txid:") }
            compose.onNodeWithText("Coins").performClick()
            compose.onNodeWithText(output.label).performScrollTo().performClick()
            compose.waitUntil(10_000) { !vm.state.value.busy && vm.state.value.outputSource != null }
            val source = checkNotNull(vm.state.value.outputSource)
            assertEquals(draft, source.id); assertEquals(2, source.inputs.size)
            compose.onNodeWithText("Created by ${source.label}").assertIsDisplayed()
            compose.onNodeWithText("Original input labels · historical record").assertIsDisplayed()
            for (input in source.inputs) {
                compose.onNodeWithText("${input.label.ifEmpty { "Unlabeled input" }} · ${formatBalance(input.sats)} BTC")
                    .performScrollTo().assertIsDisplayed()
            }
            compose.onNodeWithText("Label").performTextReplacement("Public screen user edit")
            compose.onNodeWithText("Save", substring = false).performClick()
            idle(); sync()
            compose.onNodeWithText("Public screen user edit").assertIsDisplayed()
            assertEquals(1, posts())
            StorageVault.openActive(application, root, alias).use {
                assertEquals(ChainObservation.MEMPOOL, it.broadcastStatus(wallet, draft)!!.observation)
                assertEquals("Public screen user edit", it.coins(wallet).single { coin -> coin.outpoint == output.outpoint }.label)
            }
        } catch (failure: Throwable) {
            capturePublicFixtureScreenshot("published-recovery-screen")
            throw failure
        } finally {
            compose.runOnUiThread { views.clear() }
            File(root, "restored-stores").listFiles()?.filter { it.isDirectory }?.forEach { keys.deleteEntry("$alias.${it.name}") }
            keys.deleteEntry(alias); root.deleteRecursively()
        }
    }
}

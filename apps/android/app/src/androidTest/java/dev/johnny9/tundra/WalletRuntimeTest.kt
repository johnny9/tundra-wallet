@file:OptIn(ExperimentalUnsignedTypes::class)
package dev.johnny9.tundra

import android.content.Context
import android.net.Uri
import androidx.lifecycle.ViewModelProvider
import androidx.test.core.app.ActivityScenario
import androidx.test.core.app.ApplicationProvider
import androidx.test.platform.app.InstrumentationRegistry
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createEmptyComposeRule
import dev.johnny9.tundra.generated.*
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test

class WalletRuntimeTest {
    @get:Rule val compose = createEmptyComposeRule()
    private fun fixture() = InstrumentationRegistry.getInstrumentation().context.assets
        .open("single-sig.txt").bufferedReader().use { it.readText().trim() }
    private fun waitText(text: String) {
        compose.waitUntil(10_000) { compose.onAllNodesWithText(text).fetchSemanticsNodes().isNotEmpty() }
    }
    @Test fun importRecreateReceiveAppearanceAndSyncConsent() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        // Test app sandbox only, before any engine/activity is opened.
        listOf("tundra.sqlite", "tundra.sqlite-wal", "tundra.sqlite-shm").forEach {
            context.noBackupFilesDir.resolve(it).delete()
        }
        ActivityScenario.launch(MainActivity::class.java).use { activity ->
            waitText("Add wallet")
            compose.onNodeWithText("Add wallet").performClick()
            compose.onNodeWithText("Scan descriptor QR").performClick()
            waitText("Close scan")
            compose.onNodeWithText("Close scan").performClick()
            compose.onNodeWithText("Regtest").performClick()
            compose.onNodeWithText("Or paste a public descriptor").performTextInput(fixture())
            compose.onNodeWithText("Review wallet").performClick()
            waitText("Single signature")
            compose.onNodeWithTag("confirmImport").performScrollTo().performClick()
            // Import removes the preview asynchronously; wait for the new wallet first.
            compose.waitUntil(10_000) { compose.onAllNodesWithText("— BTC").fetchSemanticsNodes().isNotEmpty() }
            compose.onNodeWithTag("closeImport").performScrollTo().performClick()
            compose.onNodeWithText("Not synced · balance unknown").assertIsDisplayed()
            compose.onNodeWithText("Receive").performClick()
            waitText("Index 0. Hardware verification is not available. Do not fund this address.")
            compose.onNodeWithText("USB address comparison").performClick()
            waitText("USB hardware")
            compose.onNodeWithText("No Ledger HID device connected. Use a USB data cable and refresh.").assertIsDisplayed()
            compose.onNodeWithText("Check public account").assertIsNotEnabled()
            compose.onNodeWithText("Compare receive address").assertIsNotEnabled()
            compose.onNodeWithText("Close USB").performScrollTo().performClick()
            activity.recreate()
            waitText("Receive")
            compose.onNodeWithText("Receive").performClick()
            waitText("Index 1. Hardware verification is not available. Do not fund this address.")
            compose.onNodeWithText("Close").performClick()
            compose.onNodeWithContentDescription("Settings").performClick()
            compose.onNodeWithText("Dark appearance").assertIsDisplayed()
            compose.onNode(isToggleable()).performClick()
            compose.onNodeWithText("Dark appearance").assertIsDisplayed()
        }
        // Reopen the real Android ABI through FFI, with no process-local BDK wallet retained.
        Tundra.open(context.noBackupFilesDir.resolve("tundra.sqlite").path).use { core ->
            val wallet = core.wallets().single()
            assertNull(wallet.totalSats)
            assertEquals(2u, core.receiveAddress(wallet.id).index)
            val op = core.prepareSync(wallet.id, "https://unused.invalid", true)
            assertEquals(SyncState.PREPARED, op.state)
            assertEquals(SyncState.CANCELLED, core.cancelSync(op.id).state)
            assertNull(core.wallets().single().syncedAt)
        }
        val recipient = Tundra.open(":memory:").use { core ->
            val descriptor = InstrumentationRegistry.getInstrumentation().context.assets
                .open("two-of-three.txt").bufferedReader().use { it.readText() }
            core.previewImport(descriptor, Chain.REGTEST).firstAddress
        }
        ActivityScenario.launch(MainActivity::class.java).use { activity ->
            waitText("Sync test network")
            compose.onNodeWithText("Sync test network").performClick()
            compose.onNodeWithText("Esplora API URL").performTextInput("http://127.0.0.1:3002")
            compose.onNodeWithText("Start scan").assertIsNotEnabled()
            compose.onNode(isToggleable()).performClick()
            compose.onNodeWithText("Start scan").performScrollTo().performClick()
            compose.waitUntil(30_000) { compose.onAllNodesWithText("150 BTC").fetchSemanticsNodes().isNotEmpty() }
            compose.onNodeWithText("Coins").performClick()
            val choices = compose.onAllNodes(hasContentDescription("Select coin", substring = true))
            choices[0].performClick(); choices[1].performClick()
            compose.onNodeWithText("Send").performClick()
            compose.onNodeWithText("Recipient address").performTextInput(recipient)
            compose.onNodeWithText("Amount in BTC").performTextInput("0.001")
            compose.onNodeWithText("Fee rate in sat/vB").performTextReplacement("2.5")
            compose.onNodeWithTag("buildReview").performScrollTo().performClick()
            waitText("Inputs · 2")
            waitText("Input 1: 0 / 1")
            compose.onNodeWithText("Import signed PSBT").performScrollTo().assertIsDisplayed()
            // Export a real reviewed draft in both formats, decode native barcode pixels,
            // then reassemble through Rust. Transport cannot add signatures.
            Tundra.open(context.noBackupFilesDir.resolve("tundra.sqlite").path).use { core ->
                val wallet = core.wallets().single()
                val draft = core.drafts(wallet.id).single()
                assertNull(core.finalizedDraft(wallet.id, draft.id))
                try { core.finalizeDraft(wallet.id, draft.id); fail("An unsigned draft cannot be finalized") }
                catch (_: AppException.Operation) { }
                assertEquals("unsigned", core.drafts(wallet.id).single().state)
                val expected = android.util.Base64.decode(core.exportSigningPsbt(wallet.id, draft.id), android.util.Base64.DEFAULT)
                for (encoding in listOf(QrEncoding.UR, QrEncoding.BBQR)) {
                    QrScanner(QrPurpose.SignedPsbt).use { scanner ->
                        for (frame in core.exportDraftQr(wallet.id, draft.id, encoding)) {
                            if (scanner.receive(decodeQrMatrix(frame)).state == QrState.COMPLETE) break
                        }
                        assertArrayEquals(expected, scanner.payload())
                    }
                }
                assertTrue(core.signingProgress(wallet.id, draft.id).inputs.all { it.validSignatures == 0u })
            }
            // Exercise the actual native file reader and byte-array FFI using an unrelated
            // published response. Its valid signatures must never be counted for this draft.
            val responseFile = context.cacheDir.resolve("public-response.psbt")
            InstrumentationRegistry.getInstrumentation().context.assets.open("hwi-signed-wpkh.psbt").use { input ->
                responseFile.outputStream().use { output -> input.copyTo(output) }
            }
            activity.onActivity { host ->
                val vm = ViewModelProvider(host)[WalletViewModel::class.java]
                val review = checkNotNull(vm.state.value.review)
                vm.importSignedDraft(Uri.fromFile(responseFile), review.walletId, review.id)
            }
            waitText("Invalid input: signing response does not match the approved draft")
            compose.onNodeWithText("Input 1: 0 / 1").performScrollTo().assertIsDisplayed()
            responseFile.delete()
            compose.onNodeWithText("Save for later").performScrollTo().performClick()
            activity.recreate()
            waitText("Activity")
            compose.onNodeWithText("Activity").performClick()
            waitText("Draft · unsigned · 2 inputs")
        }
        Tundra.open(context.noBackupFilesDir.resolve("tundra.sqlite").path).use { core ->
            val wallet = core.wallets().single()
            val draft = core.drafts(wallet.id).single()
            assertEquals(2, draft.inputs.size)
            assertEquals(625uL, draft.feeSatPerKwu)
            assertEquals(2, core.coins(wallet.id).count { it.state == CoinState.RESERVED })
            assertEquals(draft.inputs.sumOf { it.sats }, draft.outputs.sumOf { it.sats } + draft.feeSats)
            val signing = core.signingProgress(wallet.id, draft.id)
            assertFalse(signing.complete)
            assertTrue(signing.inputs.all { it.validSignatures == 0u && it.requiredSignatures == 1u })
            assertEquals(core.exportUnsignedPsbt(wallet.id, draft.id), core.exportSigningPsbt(wallet.id, draft.id))
        }
    }
}

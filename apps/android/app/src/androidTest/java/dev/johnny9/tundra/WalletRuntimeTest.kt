@file:OptIn(ExperimentalUnsignedTypes::class)
package dev.johnny9.tundra

import android.content.Context
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
            compose.onNodeWithText("Or paste a public descriptor").performTextInput(fixture())
            compose.onNodeWithText("Review wallet").performClick()
            waitText("Single signature")
            compose.onNode(hasText("Add wallet") and hasClickAction()).performClick()
            // Import removes the preview asynchronously; wait for the new wallet first.
            compose.waitUntil(10_000) { compose.onAllNodesWithText("— BTC").fetchSemanticsNodes().isNotEmpty() }
            compose.onNodeWithText("Close").performClick()
            compose.onNodeWithText("Not synced · balance unknown").assertIsDisplayed()
            compose.onNodeWithText("Receive").performClick()
            waitText("Index 0. Hardware verification is not available. Do not fund this address.")
            compose.onNodeWithText("Close").performClick()
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
    }
}

package dev.johnny9.tundra

import android.app.Application
import androidx.activity.ComponentActivity
import androidx.compose.runtime.*
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.unit.Density
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelStore
import androidx.test.core.app.ApplicationProvider
import androidx.test.platform.app.InstrumentationRegistry
import dev.johnny9.tundra.generated.*
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import java.io.File
import java.security.KeyStore
import java.util.UUID

/** The existing keyless regtest node funds 100 outputs to this public descriptor. */
class LongListRuntimeTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    @Test fun independentLongListsRetainOffsetsAndTabsStayUsableWithLargeText() {
        val application = ApplicationProvider.getApplicationContext<Application>()
        val id = UUID.randomUUID().toString()
        val root = File(application.cacheDir, "long-list-ui-$id").also { check(it.mkdir()) }
        val alias = "tundra.test.long-list.$id"
        val keys = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val views = ViewModelStore()
        try {
            val descriptor = InstrumentationRegistry.getInstrumentation().context.assets.open("two-of-three.txt")
                .bufferedReader().use { it.readText().trim() }
            StorageVault.openActive(application, root, alias).use { core ->
                core.importWallet("Public long-list wallet", descriptor, Chain.REGTEST)
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
            compose.runOnUiThread { vm.sync("http://127.0.0.1:3002", true) }
            compose.waitUntil(60_000) { vm.state.value.let { !it.busy && (it.sync?.state == SyncState.COMPLETE || it.error != null) } }
            assertNull("Public long-list sync failed", vm.state.value.error)
            assertEquals(SyncState.COMPLETE, vm.state.value.sync?.state)
            assertEquals(100, vm.state.value.coins.size)
            assertEquals(100, vm.state.value.activity.size)
            var fontScale by mutableFloatStateOf(1f)
            compose.setContent {
                val state by vm.state.collectAsState()
                CompositionLocalProvider(LocalDensity provides Density(LocalDensity.current.density, fontScale)) {
                    TundraTheme(state.dark) { WalletApp(vm, state) }
                }
            }
            fun position(tag: String): Float = compose.onNodeWithTag(tag).fetchSemanticsNode()
                .config[SemanticsProperties.VerticalScrollAxisRange].value()
            val tabsTop = compose.onNodeWithText("Coins").getUnclippedBoundsInRoot().top
            compose.onNodeWithTag("activityList").performScrollToIndex(30)
            val activityPosition = position("activityList")
            assertTrue(activityPosition >= 29f)
            compose.onNodeWithText("Coins").performClick()
            compose.onNodeWithText("Receive").assertDoesNotExist()
            compose.onNodeWithTag("coinList").performScrollToIndex(60)
            compose.onNodeWithTag("coinList").performTouchInput { swipeUp() }
            val coinPosition = position("coinList")
            assertTrue(coinPosition >= 60f)
            repeat(2) {
                compose.onNodeWithText("Activity").performClick()
                assertEquals(activityPosition, position("activityList"), 0.01f)
                assertEquals(tabsTop, compose.onNodeWithText("Coins").getUnclippedBoundsInRoot().top)
                compose.onNodeWithText("Coins").performClick()
                assertEquals(coinPosition, position("coinList"), 0.01f)
                assertEquals(tabsTop, compose.onNodeWithText("Coins").getUnclippedBoundsInRoot().top)
            }
            compose.runOnUiThread { vm.appearance(false) }
            compose.waitForIdle()
            assertEquals(coinPosition, position("coinList"), 0.01f)
            capturePublicFixtureScreenshot("long-list-light")
            compose.runOnUiThread { vm.appearance(true); fontScale = 2f }
            compose.waitForIdle()
            compose.onNodeWithText("Activity").assertIsDisplayed()
            compose.onNodeWithText("Coins").assertIsDisplayed()
            compose.onNodeWithTag("coinList").assertIsDisplayed()
            val filter = compose.onNodeWithText("Filter and sort").getUnclippedBoundsInRoot()
            val select = compose.onNodeWithText("Select").getUnclippedBoundsInRoot()
            assertTrue("Filter and selection controls overlap with large text", filter.right <= select.left)
            capturePublicFixtureScreenshot("long-list-large-text")
        } catch (failure: Throwable) {
            capturePublicFixtureScreenshot("long-list-failure")
            throw failure
        } finally {
            compose.runOnUiThread { views.clear() }
            keys.deleteEntry(alias); root.deleteRecursively()
        }
    }
}

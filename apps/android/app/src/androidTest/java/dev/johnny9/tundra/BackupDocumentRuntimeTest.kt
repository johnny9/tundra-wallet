package dev.johnny9.tundra

import android.app.Application
import android.accessibilityservice.AccessibilityServiceInfo
import androidx.activity.ComponentActivity
import android.os.Bundle
import android.os.SystemClock
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityWindowInfo
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
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
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import java.io.File
import java.security.KeyStore
import java.util.UUID

/** Real document picker, provider readback, Keystore and Rust; isolated public wallet. */
class BackupDocumentRuntimeTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    private fun systemNode(predicate: (AccessibilityNodeInfo) -> Boolean): AccessibilityNodeInfo {
        val automation = InstrumentationRegistry.getInstrumentation().uiAutomation
        automation.serviceInfo = automation.serviceInfo.apply {
            flags = flags or AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS or AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS
        }
        val deadline = SystemClock.uptimeMillis() + 15_000
        while (SystemClock.uptimeMillis() < deadline) {
            val queue = java.util.ArrayDeque<AccessibilityNodeInfo>()
            automation.windows.filter { it.type == AccessibilityWindowInfo.TYPE_APPLICATION }.forEach { window ->
                window.root?.takeIf { it.packageName?.toString()?.contains("documentsui", ignoreCase = true) == true }?.let(queue::add)
            }
            if (queue.isEmpty()) automation.rootInActiveWindow?.let(queue::add)
            var visited = 0
            while (queue.isNotEmpty() && visited++ < 1024) {
                val node = queue.removeFirst()
                if (predicate(node)) return node
                for (index in 0 until node.childCount) node.getChild(index)?.let(queue::add)
            }
            SystemClock.sleep(100)
        }
        capturePublicFixtureScreenshot("backup-document-control")
        throw AssertionError("Expected system document control was not available")
    }

    private fun clickSystemText(text: String) {
        var node: AccessibilityNodeInfo? = systemNode { it.text?.toString()?.equals(text, ignoreCase = true) == true }
        repeat(6) {
            if (node?.isClickable == true && node?.performAction(AccessibilityNodeInfo.ACTION_CLICK) == true) return
            node = node?.parent
        }
        throw AssertionError("Expected system document control was not clickable")
    }

    @Test fun encryptedDocumentSaveReadbackAndReviewedRestoreUseNewProtectedGeneration() {
        val application = ApplicationProvider.getApplicationContext<Application>()
        val fixture = InstrumentationRegistry.getInstrumentation().context.assets
            .open("single-sig.txt").bufferedReader().use { it.readText() }
        val id = UUID.randomUUID().toString()
        val root = File(application.cacheDir, "backup-document-$id").also { check(it.mkdir()) }
        val alias = "tundra.test.documents.$id"
        val keys = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val views = ViewModelStore()
        try {
            StorageVault.openActive(application, root, alias).use { core ->
                val wallet = core.importWallet("Public document fixture", fixture, Chain.SIGNET)
                val address = core.receiveAddress(wallet.id)
                core.setLabel(wallet.id, "addr", address.address, "Document retained label")
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
            compose.setContent {
                val state by vm.state.collectAsState()
                TundraTheme(true) { BackupSheet(vm, state) {} }
            }
            val password = "Public document test password 2026"
            compose.onNodeWithTag("backupPassword").performScrollTo().performTextInput(password)
            compose.onNodeWithTag("backupPasswordConfirmation").performScrollTo().performTextInput("different public password")
            compose.onNodeWithTag("prepareBackup").assertIsNotEnabled()
            compose.onNodeWithTag("backupPasswordConfirmation").performTextReplacement(password)
            compose.onNodeWithTag("backupPasswordConfirmation").performImeAction()
            compose.waitUntil(10_000) { compose.runOnUiThread {
                ViewCompat.getRootWindowInsets(compose.activity.window.decorView)?.isVisible(WindowInsetsCompat.Type.ime()) != true
            } }
            compose.onNodeWithTag("prepareBackup").performScrollTo().assertIsDisplayed().assertIsEnabled().performClick()
            compose.waitUntil(10_000) { vm.state.value.backupExportReady || vm.state.value.error != null }
            assertNull("Public fixture export preparation failed", vm.state.value.error)
            // StateFlow readiness precedes the recomposition/LaunchedEffect that
            // launches DocumentsUI. Flush the Compose test clock before native polling.
            compose.waitForIdle()
            val filename = "tundra-public-$id.tundra"
            val name = systemNode { it.className?.toString()?.endsWith("EditText") == true && it.text?.contains("tundra-backup") == true }
            assertTrue(name.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, Bundle().apply {
                putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, filename)
            }))
            clickSystemText("Save")
            compose.waitUntil(20_000) { !vm.state.value.busy && vm.state.value.backupMessage != null }
            assertTrue(vm.state.value.backupMessage?.startsWith("Encrypted backup saved and read back successfully") == true)
            compose.onNodeWithText("Restore", useUnmergedTree = true).performScrollTo().performClick()
            compose.onNodeWithTag("backupPassword").performScrollTo().performTextInput(password)
            compose.onNodeWithText("Choose backup file").performScrollTo().performClick()
            clickSystemText(filename)
            compose.waitUntil(20_000) { vm.state.value.backupPreview != null && !vm.state.value.busy }
            compose.onNodeWithTag("restoreBackup").assertIsNotEnabled()
            compose.onNodeWithText("Public document fixture · Single signature").assertExists()
            compose.onNodeWithTag("backupRestoreConsent").performScrollTo().performClick()
            compose.onNodeWithTag("restoreBackup").performScrollTo().performClick()
            compose.waitUntil(20_000) { vm.state.value.storageReady && !vm.state.value.busy && vm.state.value.backupMessage?.startsWith("Backup restored.") == true }
            val selected = selectedStorage(root.path)
            assertNotEquals("default", selected.generation)
            assertNull(vm.state.value.wallet?.totalSats)
            assertNull(vm.state.value.wallet?.syncedAt)
            StorageVault.openActive(application, root, alias).use { core ->
                val wallet = core.wallets().single()
                assertEquals(1u, core.receiveAddress(wallet.id).index)
                assertTrue(core.exportLabels(wallet.id).contains("Document retained label"))
            }
            assertTrue(File(root, "tundra.sqlite").isFile)
        } finally {
            val automation = InstrumentationRegistry.getInstrumentation().uiAutomation
            if (automation.rootInActiveWindow?.packageName?.toString()?.contains("documentsui", ignoreCase = true) == true) {
                automation.performGlobalAction(android.accessibilityservice.AccessibilityService.GLOBAL_ACTION_BACK)
            }
            compose.runOnUiThread { views.clear() }
            File(root, "restored-stores").listFiles()?.forEach { keys.deleteEntry("$alias.${it.name}") }
            keys.deleteEntry(alias); root.deleteRecursively()
        }
    }
}

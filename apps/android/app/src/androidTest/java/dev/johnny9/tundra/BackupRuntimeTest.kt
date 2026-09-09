package dev.johnny9.tundra

import androidx.test.platform.app.InstrumentationRegistry
import dev.johnny9.tundra.generated.*
import org.junit.Assert.*
import org.junit.Test
import java.io.File
import java.util.UUID

class BackupRuntimeTest {
    @Test fun portableEncryptedSnapshotAndNativeExportCrossMobileFfi() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val directory = File(instrumentation.targetContext.cacheDir, "backup-test-${UUID.randomUUID()}")
        check(directory.mkdir())
        try {
            val password = "Public backup test password 🧊 ' spaces "
            val imported = File(directory, "imported.tundra")
            instrumentation.context.assets.open("backup-v1.tundra").use { input -> imported.outputStream().use { input.copyTo(it) } }
            val info = inspectBackup(imported.path, password)
            assertEquals("Backup public fixture", info.wallets.single().name)
            assertFalse(info.wallets.single().synced)
            assertNull(info.wallets.single().totalSats)
            val before = imported.readBytes()
            assertThrows(AppException.Operation::class.java) { inspectBackup(imported.path, "incorrect public test password") }
            assertArrayEquals(before, imported.readBytes())
            val restored = File(directory, "restored.sqlite")
            restoreBackup(imported.path, restored.path, password, ByteArray(32) { 0x33 })
            Tundra.openProtected(restored.path, ByteArray(32) { 0x33 }).use { core ->
                val wallet = core.wallets().single()
                assertFalse(wallet.synced); assertNull(wallet.totalSats)
                assertEquals(1u, core.receiveAddress(wallet.id).index)
                assertTrue(core.exportLabels(wallet.id).contains("Backup private metadata sentinel"))
            }
            assertArrayEquals(before, imported.readBytes())
            val fixture = instrumentation.context.assets.open("single-sig.txt").bufferedReader().use { it.readText() }
            Tundra.openProtected(File(directory, "wallet.sqlite").path, ByteArray(32) { 0x11 }).use { core ->
                val wallet = core.importWallet("Mobile backup fixture", fixture, Chain.SIGNET)
                val output = File(directory, "exported.tundra")
                assertEquals(wallet.id, core.exportBackup(output.path, password).wallets.single().id)
                assertEquals(wallet.id, inspectBackup(output.path, password).wallets.single().id)
            }
        } finally { directory.deleteRecursively() }
    }
}

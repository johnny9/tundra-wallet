package dev.johnny9.tundra

import androidx.test.platform.app.InstrumentationRegistry
import dev.johnny9.tundra.generated.*
import org.junit.Assert.*
import org.junit.Test
import java.io.File
import java.util.UUID

class StorageRuntimeTest {
    @Test fun encryptedDatabaseCrossesMobileFfiAndWrongKeyPreservesIt() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val directory = File(instrumentation.targetContext.cacheDir, "storage-test-${UUID.randomUUID()}")
        check(directory.mkdir())
        try {
            val file = File(directory, "protected.sqlite")
            val fixture = instrumentation.context.assets.open("single-sig.txt").bufferedReader().use { it.readText() }
            // Public database-encryption fixture, not a Bitcoin signing key or Keystore test.
            val key = ByteArray(32) { 0x11 }
            val id = Tundra.openProtected(file.path, key).use { core ->
                val wallet = core.importWallet("Protected public fixture", fixture, Chain.SIGNET)
                val address = core.receiveAddress(wallet.id)
                core.setLabel(wallet.id, "addr", address.address, "Protected public label 🧊")
                wallet.id
            }
            val before = file.readBytes()
            assertFalse(before.toString(Charsets.ISO_8859_1).contains("Protected public fixture"))
            val error = assertThrows(AppException.Operation::class.java) {
                Tundra.openProtected(file.path, ByteArray(32) { 0x22 }).close()
            }
            assertEquals(ErrorCode.STORAGE, error.code)
            assertFalse(error.detail.contains(file.path))
            assertThrows(AppException.Operation::class.java) { Tundra.open(file.path).close() }
            assertArrayEquals(before, file.readBytes())
            Tundra.openProtected(file.path, key).use { core ->
                assertEquals(id, core.wallets().single().id)
                assertNull(core.wallets().single().totalSats)
                assertEquals(1u, core.receiveAddress(id).index)
                assertTrue(core.exportLabels(id).contains("Protected public label 🧊"))
            }
        } finally { directory.deleteRecursively() }
    }
}

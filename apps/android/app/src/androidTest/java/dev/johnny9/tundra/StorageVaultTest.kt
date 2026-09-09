package dev.johnny9.tundra

import androidx.test.platform.app.InstrumentationRegistry
import dev.johnny9.tundra.generated.*
import org.junit.Assert.*
import org.junit.Test
import java.io.File
import java.security.KeyStore
import java.util.UUID

/** Uses Android Keystore with storage keys and disposable public watch-only fixtures. */
class StorageVaultTest {
    private fun fixture() = InstrumentationRegistry.getInstrumentation().context.assets
        .open("single-sig.txt").bufferedReader().use { it.readText() }
    private fun isolated(test: (android.content.Context, File, String, KeyStore) -> Unit) {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val id = UUID.randomUUID().toString()
        val directory = File(context.cacheDir, "vault-test-$id")
        check(directory.mkdir())
        val alias = "tundra.test.storage.$id"
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        try { test(context, directory, alias, store) }
        finally {
            File(directory, "restored-stores").listFiles()?.filter { it.isDirectory }?.forEach {
                store.deleteEntry("$alias.${it.name}") // Only this isolated test's generation keys.
            }
            store.deleteEntry(alias); directory.deleteRecursively()
        }
    }

    @Test fun keystoreReopenAndLostOrTamperedKeyMaterialNeverResetWallet() = isolated { context, directory, alias, store ->
        val id = StorageVault.open(context, directory, alias).use { core ->
            val wallet = core.importWallet("Keystore public fixture", fixture(), Chain.SIGNET)
            val address = core.receiveAddress(wallet.id)
            core.setLabel(wallet.id, "addr", address.address, "Keystore retained label 🧊")
            wallet.id
        }
        assertTrue(store.getKey(alias, null).encoded == null)
        val database = File(directory, "tundra.sqlite")
        val wrapped = File(directory, "storage-key.v1")
        val packet = wrapped.readBytes()
        assertEquals(65, packet.size)
        StorageVault.open(context, directory, alias).use { core ->
            assertEquals(id, core.wallets().single().id)
            assertEquals(1u, core.receiveAddress(id).index)
            assertTrue(core.exportLabels(id).contains("Keystore retained label 🧊"))
        }
        assertArrayEquals(packet, wrapped.readBytes())
        val before = database.readBytes()
        val changed = packet.copyOf().also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() }
        for (invalid in listOf(changed, byteArrayOf(), packet + byteArrayOf(0))) {
            wrapped.writeBytes(invalid)
            assertThrows(StorageAccessException::class.java) { StorageVault.open(context, directory, alias).close() }
            assertArrayEquals(invalid, wrapped.readBytes())
            assertArrayEquals(before, database.readBytes())
        }
        wrapped.writeBytes(packet)
        check(wrapped.delete())
        assertThrows(StorageAccessException::class.java) { StorageVault.open(context, directory, alias).close() }
        assertFalse(wrapped.exists())
        assertArrayEquals(before, database.readBytes())
        wrapped.writeBytes(packet)
        store.deleteEntry(alias)
        assertThrows(StorageAccessException::class.java) { StorageVault.open(context, directory, alias).close() }
        assertFalse(store.containsAlias(alias))
        assertArrayEquals(packet, wrapped.readBytes())
        assertArrayEquals(before, database.readBytes())
    }

    @Test fun keystoreUpgradeRetainsLegacyDataAndInitializedMarkerRefusesMissingDatabase() = isolated { context, directory, alias, _ ->
        val database = File(directory, "tundra.sqlite")
        val id = Tundra.open(database.path).use { core ->
            val wallet = core.importWallet("Legacy Keystore fixture", fixture(), Chain.SIGNET)
            core.receiveAddress(wallet.id)
            wallet.id
        }
        assertEquals(StorageFile.LEGACY_PLAINTEXT, inspectStorage(database.path))
        StorageVault.open(context, directory, alias).use { core ->
            assertEquals(id, core.wallets().single().id)
            assertEquals(1u, core.receiveAddress(id).index)
        }
        assertEquals(StorageFile.PROTECTED_OR_UNKNOWN, inspectStorage(database.path))
        val wrapped = File(directory, "storage-key.v1")
        val retained = wrapped.readBytes()
        check(database.delete()) // Isolated test-only file, all native handles closed.
        assertThrows(StorageAccessException::class.java) { StorageVault.open(context, directory, alias).close() }
        assertFalse(database.exists())
        assertArrayEquals(retained, wrapped.readBytes())
    }

    @Test fun retainedMasterWithoutDatabaseOrWrapperCannotLookLikeNewInstallation() = isolated { context, directory, alias, store ->
        StorageVault.open(context, directory, alias).close()
        check(File(directory, "tundra.sqlite").delete())
        check(File(directory, "storage-key.v1").delete())
        assertTrue(store.containsAlias(alias))
        assertThrows(StorageAccessException::class.java) { StorageVault.open(context, directory, alias).close() }
        assertFalse(File(directory, "tundra.sqlite").exists())
        assertFalse(File(directory, "storage-key.v1").exists())
        assertTrue(store.containsAlias(alias))
    }

    @Test fun restoreSwitchesRetainedKeystoreGenerationsAndRecoversFromLostKeysAndSelectors() = isolated { context, directory, alias, store ->
        val source = File(directory, "public-backup.tundra")
        InstrumentationRegistry.getInstrumentation().context.assets.open("backup-v1.tundra").use { input ->
            source.outputStream().use { input.copyTo(it) }
        }
        val password = "Public backup test password 🧊 ' spaces "
        val backupBytes = source.readBytes()
        val oldId = StorageVault.openActive(context, directory, alias).use {
            it.importWallet("Original generation", fixture(), Chain.SIGNET).id
        }
        val oldDatabase = File(directory, "tundra.sqlite")
        val oldBytes = oldDatabase.readBytes()
        assertThrows(StorageAccessException::class.java) {
            StorageVault.restoreActive(context, source, "Incorrect public password", directory, alias).close()
        }
        StorageVault.openActive(context, directory, alias).use { assertEquals(oldId, it.wallets().single().id) }
        StorageVault.restoreActive(context, source, password, directory, alias).use {
            val wallet = it.wallets().single()
            assertEquals("Backup public fixture", wallet.name)
            assertNull(wallet.totalSats); assertNull(wallet.syncedAt)
            assertEquals(1u, it.receiveAddress(wallet.id).index)
        }
        var location = selectedStorage(directory.path)
        assertNotEquals("default", location.generation)
        assertTrue(location.requireExisting)
        StorageVault.openActive(context, directory, alias).use {
            assertEquals(2u, it.receiveAddress(it.wallets().single().id).index)
        }
        assertArrayEquals(oldBytes, oldDatabase.readBytes())
        val broken = File(location.directory, "tundra.sqlite")
        val brokenBytes = broken.readBytes()
        store.deleteEntry("$alias.${location.generation}")
        assertThrows(StorageAccessException::class.java) { StorageVault.openActive(context, directory, alias).close() }
        StorageVault.restoreActive(context, source, password, directory, alias).close()
        assertNotEquals(location.generation, selectedStorage(directory.path).generation)
        assertArrayEquals(brokenBytes, broken.readBytes())
        File(directory, "active-store.v1").writeText("corrupt selector")
        assertThrows(StorageAccessException::class.java) { StorageVault.openActive(context, directory, alias).close() }
        StorageVault.restoreActive(context, source, password, directory, alias).close()
        check(File(directory, "active-store.v1").delete())
        assertThrows(StorageAccessException::class.java) { StorageVault.openActive(context, directory, alias).close() }
        StorageVault.restoreActive(context, source, password, directory, alias).close()
        location = selectedStorage(directory.path)
        val selectedDatabase = File(location.directory, "tundra.sqlite")
        check(selectedDatabase.delete()) // Disposable generation; test must prove no re-creation.
        assertThrows(StorageAccessException::class.java) { StorageVault.openActive(context, directory, alias).close() }
        assertFalse(selectedDatabase.exists())
        assertArrayEquals(backupBytes, source.readBytes())
        assertArrayEquals(oldBytes, oldDatabase.readBytes())
    }
}

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
    @Test fun publishedSignaturesRestoreReviewSubmissionAndProvenanceCrossNativeVault() = isolated { context, directory, alias, _ ->
        val assets = InstrumentationRegistry.getInstrumentation().context.assets
        val fixture = org.json.JSONObject(assets.open("native-signing.json").bufferedReader().use { it.readText() })
        val wallet = fixture.getString("wallet_id")
        val draft = fixture.getString("draft_id")
        val transaction = fixture.getJSONObject("final")
        val txid = transaction.getString("txid")
        val signed = assets.open("native-signed-response.psbt").use { it.readBytes() }
        val database = File(directory, "tundra.sqlite")
        assets.open("native-unsigned.sqlite").use { input -> database.outputStream().use { input.copyTo(it) } }
        fun exact(value: FinalTransactionInfo) {
            assertEquals(txid, value.txid)
            assertEquals(transaction.getString("wtxid"), value.wtxid)
            assertEquals(transaction.getString("raw"), value.transactionBytes.joinToString("") { "%02x".format(it) })
        }
        StorageVault.openActive(context, directory, alias).use { core ->
            assertEquals("unsigned", core.drafts(wallet).single().state)
            assertFalse(core.signingProgress(wallet, draft).complete)
            assertThrows(AppException.Operation::class.java) { core.finalizeDraft(wallet, draft) }
            val progress = core.acceptSignedPsbt(wallet, draft, signed)
            assertTrue(progress.complete); assertEquals(2, progress.inputs.size)
            assertTrue(progress.inputs.all { it.validSignatures == it.requiredSignatures })
            exact(core.finalizeDraft(wallet, draft))
            assertNull(core.broadcastStatus(wallet, draft))
        }
        assertEquals(StorageFile.PROTECTED_OR_UNKNOWN, inspectStorage(database.path))
        StorageVault.openActive(context, directory, alias).use { exact(it.finalizedDraft(wallet, draft)!!) }
        val backup = File(directory, "public-signed.tundra")
        assets.open("native-signed-backup.tundra").use { input -> backup.outputStream().use { input.copyTo(it) } }
        val endpoint = "http://127.0.0.1:3003" // Synthetic local fixture, NOT a Signet broadcast.
        fun sync(core: Tundra) {
            val operation = core.prepareSync(wallet, endpoint, true)
            core.runSync(operation.id)
            val deadline = android.os.SystemClock.uptimeMillis() + 30_000
            while (true) {
                val progress = core.syncProgress(operation.id)
                if (progress.state == SyncState.COMPLETE) break
                assertFalse("Public fixture scan failed", progress.state in listOf(SyncState.FAILED, SyncState.CANCELLED))
                assertTrue("Public fixture scan timed out", android.os.SystemClock.uptimeMillis() < deadline)
                android.os.SystemClock.sleep(10)
            }
        }
        StorageVault.restoreActive(context, backup, fixture.getString("password"), directory, alias).use { core ->
            assertNull(core.wallets().single().totalSats)
            assertEquals("invalidated", core.drafts(wallet).single().state)
            assertTrue(core.recoveryRequired(wallet, draft))
            var review = RecoveryReviewRequest(wallet, draft, txid, 1uL, true)
            assertThrows(AppException.Operation::class.java) { core.resumeRecoveredSubmission(review) }
            sync(core)
            assertEquals(2, core.coins(wallet).count { it.state == CoinState.RESERVED })
            review = review.copy(reviewAcknowledged = false)
            assertThrows(AppException.Operation::class.java) { core.resumeRecoveredSubmission(review) }
            exact(core.resumeRecoveredSubmission(review.copy(reviewAcknowledged = true)))
            assertFalse(core.recoveryRequired(wallet, draft))
            assertFalse(core.broadcastStatus(wallet, draft)!!.acknowledged)
            var request = BroadcastRequest(wallet, draft, endpoint, txid, 1uL, false, false)
            assertThrows(AppException.Operation::class.java) { core.broadcastDraft(request) }
            request = request.copy(privacyConsent = true)
            assertThrows(AppException.Operation::class.java) { core.broadcastDraft(request) }
            val sent = core.broadcastDraft(request.copy(retryAcknowledged = true))
            assertTrue(sent.acknowledged); assertEquals(ChainObservation.NOT_SEEN, sent.observation)
            assertEquals(2uL, sent.attemptId)
        }
        assertNotEquals("default", selectedStorage(directory.path).generation)
        StorageVault.openActive(context, directory, alias).use { core ->
            assertEquals(2uL, core.broadcastStatus(wallet, draft)!!.attemptId)
            assertTrue(core.broadcastStatus(wallet, draft)!!.acknowledged)
            sync(core)
            assertEquals(ChainObservation.MEMPOOL, core.broadcastStatus(wallet, draft)!!.observation)
            assertEquals("observed", core.drafts(wallet).single().state)
            assertFalse(core.coins(wallet).any { it.state == CoinState.RESERVED })
            val output = core.coins(wallet).single { it.outpoint.startsWith("$txid:") }
            val source = core.outputSource(wallet, output.outpoint)!!
            assertEquals(draft, source.id); assertEquals(2, source.inputs.size)
            assertEquals(source.label, output.label)
            core.setLabel(wallet, "output", output.outpoint, "Public native user edit")
            sync(core)
            assertEquals("Public native user edit", core.coins(wallet).single { it.outpoint == output.outpoint }.label)
        }
    }

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
        var oldBytes = oldDatabase.readBytes()
        assertThrows(StorageAccessException::class.java) {
            StorageVault.restoreActive(context, source, "Incorrect public password", directory, alias).close()
        }
        assertArrayEquals(oldBytes, oldDatabase.readBytes())
        StorageVault.openActive(context, directory, alias).use { assertEquals(oldId, it.wallets().single().id) }
        // A legitimate core reopen may rewrite SQLCipher pages (fresh IVs). Snapshot
        // after that reopen so this checks mutations by subsequent restore operations.
        oldBytes = oldDatabase.readBytes()
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

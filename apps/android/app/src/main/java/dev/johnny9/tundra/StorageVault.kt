package dev.johnny9.tundra

import android.app.KeyguardManager
import android.content.Context
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.system.Os
import android.system.OsConstants
import dev.johnny9.tundra.generated.*
import java.io.File
import java.io.FileOutputStream
import java.io.RandomAccessFile
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

class StorageAccessException : Exception("Wallet storage is unavailable. Existing files were retained; unlock the device or use recovery.")

/** Storage encryption only. Never generates or retains Bitcoin signing keys. */
internal object StorageVault {
    private const val APP_ALIAS = "dev.johnny9.tundra.storage.v1"
    private val header = byteArrayOf(0x54, 0x44, 0x53, 1)
    private class Record(val key: ByteArray, val initialized: Boolean)

    @Synchronized
    fun openActive(context: Context, directory: File = context.noBackupFilesDir, alias: String = APP_ALIAS): Tundra {
        try {
            check(directory.isDirectory || directory.mkdirs())
            val location = selectedStorage(directory.path)
            return open(context, File(location.directory), generationAlias(alias, location.generation),
                requireExisting = location.requireExisting)
        } catch (_: Exception) { throw StorageAccessException() }
    }

    /** The caller must finish active network/USB work and close its old core first.
     * If activation reports an error, remain unavailable and reopen selection; never
     * keep using an old handle because the atomic selector replacement may have won. */
    @Synchronized
    fun restoreActive(context: Context, source: File, password: String,
                      directory: File = context.noBackupFilesDir, alias: String = APP_ALIAS): Tundra {
        try {
            requireUnlocked(context)
            check(directory.isDirectory || directory.mkdirs())
            StoreRestoreSession.begin(directory.path, "tundra.sqlite").use { session ->
                val location = session.location()
                val candidate = File(location.directory)
                val candidateAlias = generationAlias(alias, location.generation)
                val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
                check(!store.containsAlias(candidateAlias))
                val wrapped = File(candidate, "storage-key.v1")
                check(!wrapped.exists())
                check(inspectStorage(File(candidate, "tundra.sqlite").path) == StorageFile.MISSING)
                val master = generateMaster(candidateAlias)
                val key = ByteArray(32).also { SecureRandom().nextBytes(it) }
                try {
                    write(wrapped, Record(key, false), master, candidateAlias, replacing = false)
                    session.restore(source.path, password, key)
                    // open verifies the same retained key and durably marks it initialized.
                    val restored = open(context, candidate, candidateAlias)
                    try { session.activate(); return restored }
                    catch (error: Exception) { restored.close(); throw error }
                } finally { key.fill(0) }
            }
        } catch (_: Exception) { throw StorageAccessException() }
    }

    private fun generationAlias(base: String, generation: String) =
        if (generation == "default") base else "$base.$generation"

    private fun requireUnlocked(context: Context) {
        val keyguard = context.getSystemService(KeyguardManager::class.java) ?: throw StorageAccessException()
        if (keyguard.isDeviceLocked) throw StorageAccessException()
    }

    // Serializes initialization within the app process; the file lock covers a second
    // process. Rust separately excludes live DB handles from plaintext migration.
    @Synchronized
    fun open(context: Context, directory: File = context.noBackupFilesDir,
             alias: String = APP_ALIAS, databaseName: String = "tundra.sqlite",
             requireExisting: Boolean = false): Tundra {
        try {
            requireUnlocked(context)
            check(directory.isDirectory || directory.mkdirs())
            val root = directory.canonicalFile
            val lockFile = File(root, "storage-init.lock")
            check(!Files.isSymbolicLink(lockFile.toPath()))
            RandomAccessFile(lockFile, "rw").use { locking ->
                val lock = locking.channel.tryLock() ?: throw StorageAccessException()
                lock.use {
                    val database = File(root, databaseName)
                    val format = inspectStorage(database.path)
                    if (requireExisting && format != StorageFile.PROTECTED_OR_UNKNOWN) throw StorageAccessException()
                    val wrapped = File(root, "storage-key.v1")
                    check(!Files.isSymbolicLink(wrapped.toPath()))
                    val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
                    val record: Record
                    val master: SecretKey
                    if (wrapped.exists()) {
                        master = store.getKey(alias, null) as? SecretKey ?: throw StorageAccessException()
                        record = read(wrapped, master, alias)
                    } else {
                        // A lost key must never authorize a fresh empty wallet database.
                        if (requireExisting || format == StorageFile.PROTECTED_OR_UNKNOWN || store.containsAlias(alias)) {
                            throw StorageAccessException()
                        }
                        master = generateMaster(alias)
                        record = Record(ByteArray(32).also { SecureRandom().nextBytes(it) }, false)
                        try { write(wrapped, record, master, alias, replacing = false) }
                        catch (error: Exception) { record.key.fill(0); throw error }
                    }
                    try {
                        if (requireExisting && !record.initialized) throw StorageAccessException()
                        if (record.initialized && format != StorageFile.PROTECTED_OR_UNKNOWN) {
                            throw StorageAccessException()
                        }
                        if (format == StorageFile.LEGACY_PLAINTEXT) upgradeStorage(database.path, record.key)
                        val core = Tundra.openProtected(database.path, record.key)
                        try {
                            // Both the pending and initialized records retain the same key.
                            // Interrupted marker replacement never changes DB decryption.
                            if (!record.initialized) write(wrapped, Record(record.key, true), master, alias, replacing = true)
                            return core
                        } catch (error: Exception) { core.close(); throw error }
                    } finally { record.key.fill(0) }
                }
            }
        } catch (_: Exception) { throw StorageAccessException() }
    }

    private fun generateMaster(alias: String): SecretKey {
        val spec = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setKeySize(256).setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setRandomizedEncryptionRequired(true)
        // Android documents destructive/unlock bugs on API 31–34. Older devices use
        // app credential-encrypted storage plus the explicit keyguard check above.
        if (Build.VERSION.SDK_INT >= 35) spec.setUnlockedDeviceRequired(true)
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").run {
            init(spec.build()); generateKey()
        }
    }

    private fun aad(alias: String) = "Tundra storage wrapper v1:$alias".toByteArray(Charsets.UTF_8)
    private fun read(file: File, master: SecretKey, alias: String): Record {
        // Four header bytes, 12-byte GCM nonce, 33-byte plaintext plus a 16-byte tag.
        val bytes = file.inputStream().use { input ->
            val buffer = ByteArray(66)
            var count = 0
            while (count < buffer.size) {
                val read = input.read(buffer, count, buffer.size - count)
                if (read < 0) break
                check(read > 0)
                count += read
            }
            buffer.copyOf(count)
        }
        check(bytes.size == 65 && bytes.copyOfRange(0, 4).contentEquals(header))
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, master, GCMParameterSpec(128, bytes.copyOfRange(4, 16)))
        cipher.updateAAD(aad(alias))
        val plaintext = cipher.doFinal(bytes, 16, 49)
        try {
            check(plaintext.size == 33 && plaintext[32].toInt() in 0..1)
            return Record(plaintext.copyOfRange(0, 32), plaintext[32] == 1.toByte())
        } finally { plaintext.fill(0) }
    }

    private fun write(file: File, record: Record, master: SecretKey, alias: String, replacing: Boolean) {
        check(file.exists() == replacing)
        val plaintext = ByteArray(33)
        record.key.copyInto(plaintext)
        plaintext[32] = if (record.initialized) 1 else 0
        val ciphertext: ByteArray
        val nonce: ByteArray
        try {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, master) // Keystore supplies a fresh nonce.
            cipher.updateAAD(aad(alias))
            ciphertext = cipher.doFinal(plaintext); nonce = cipher.iv
        } finally { plaintext.fill(0) }
        check(nonce.size == 12 && ciphertext.size == 49)
        val temporary = File.createTempFile(".storage-key-", ".tmp", file.parentFile)
        try {
            FileOutputStream(temporary).use { output ->
                output.write(header); output.write(nonce); output.write(ciphertext); output.fd.sync()
            }
            Files.move(temporary.toPath(), file.toPath(), StandardCopyOption.ATOMIC_MOVE,
                *if (replacing) arrayOf(StandardCopyOption.REPLACE_EXISTING) else emptyArray())
            // O_DIRECTORY is not a public Android SDK constant. Check the opened
            // descriptor using the supported fstat API before syncing its directory.
            val directory = Os.open(checkNotNull(file.parent), OsConstants.O_RDONLY or OsConstants.O_CLOEXEC, 0)
            try {
                check(OsConstants.S_ISDIR(Os.fstat(directory).st_mode))
                Os.fsync(directory)
            } finally { Os.close(directory) }
        } finally { temporary.delete() }
    }
}

package dev.johnny9.tundra

import android.content.Context
import android.net.Uri
import android.os.ParcelFileDescriptor
import java.io.File
import java.io.FileOutputStream
import java.io.InputStream
import java.io.OutputStream
import java.security.MessageDigest
import java.util.UUID

class BackupAccessException : Exception("Backup could not be opened. Check the password and file; existing wallet files were retained.")
class BackupSaveException : Exception("The saved backup could not be verified. The selected file may be incomplete; keep another verified backup.")

/** Streams bounded document bytes; only completed core exports are sent to a provider. */
internal object BackupFiles {
    const val MAX_BYTES = 256L * 1024 * 1024

    fun directory(context: Context): File = File(context.noBackupFilesDir, "backup-staging").also {
        check(it.isDirectory || it.mkdirs())
    }
    fun exportPath(context: Context): File = File(directory(context), "export-${UUID.randomUUID()}.tundra")

    fun stage(context: Context, uri: Uri): File {
        require(uri.scheme == "content")
        val file = File.createTempFile("import-", ".tundra", directory(context))
        try {
            context.contentResolver.openInputStream(uri)?.use { input ->
                FileOutputStream(file).use { output -> copy(input, output); output.fd.sync() }
            } ?: throw BackupAccessException()
            return file
        } catch (_: Exception) { file.delete(); throw BackupAccessException() }
    }

    fun save(context: Context, source: File, destination: Uri) {
        try {
            require(destination.scheme == "content")
            val expected = source.inputStream().use { digest(it) }
            require(expected.first >= 4096)
            // CreateDocument supplied a new document. Some providers do not support
            // sync/readback; those cannot be reported as a verified saved backup.
            val descriptor = context.contentResolver.openFileDescriptor(destination, "wt") ?: throw BackupSaveException()
            ParcelFileDescriptor.AutoCloseOutputStream(descriptor).use { output ->
                source.inputStream().use { copy(it, output) }
                output.fd.sync()
            }
            val actual = context.contentResolver.openInputStream(destination)?.use { digest(it) }
                ?: throw BackupSaveException()
            check(actual.first == expected.first && MessageDigest.isEqual(actual.second, expected.second))
        } catch (_: Exception) { throw BackupSaveException() }
    }

    internal fun copy(input: InputStream, output: OutputStream, max: Long = MAX_BYTES): Long {
        require(max >= 0)
        val buffer = ByteArray(64 * 1024)
        var total = 0L
        var emptyReads = 0
        while (true) {
            val count = input.read(buffer, 0, minOf(buffer.size.toLong(), max - total + 1).toInt())
            if (count < 0) return total
            if (count == 0) { check(++emptyReads <= 16); continue }
            emptyReads = 0
            check(count.toLong() <= max - total)
            output.write(buffer, 0, count); total += count
        }
    }

    internal fun digest(input: InputStream): Pair<Long, ByteArray> {
        val hash = MessageDigest.getInstance("SHA-256")
        val count = copy(input, object : OutputStream() {
            override fun write(value: Int) { hash.update(value.toByte()) }
            override fun write(bytes: ByteArray, offset: Int, length: Int) { hash.update(bytes, offset, length) }
        })
        return count to hash.digest()
    }
}

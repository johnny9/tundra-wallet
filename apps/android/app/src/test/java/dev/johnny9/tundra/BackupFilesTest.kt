package dev.johnny9.tundra

import org.junit.Assert.*
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.io.OutputStream
import java.security.MessageDigest

class BackupFilesTest {
    @Test fun documentCopyHandlesShortReadsAndRejectsOverflowStallsAndWriteFailures() {
        val bytes = ByteArray(513) { (it % 251).toByte() }
        val short = object : ByteArrayInputStream(bytes) {
            override fun read(buffer: ByteArray, offset: Int, length: Int): Int = super.read(buffer, offset, minOf(length, 3))
        }
        val output = ByteArrayOutputStream()
        assertEquals(513L, BackupFiles.copy(short, output, 513))
        assertArrayEquals(bytes, output.toByteArray())
        assertThrows(IllegalStateException::class.java) {
            BackupFiles.copy(ByteArrayInputStream(bytes), ByteArrayOutputStream(), 512)
        }
        val stalled = object : InputStream() {
            override fun read() = 0
            override fun read(buffer: ByteArray, offset: Int, length: Int) = 0
        }
        assertThrows(IllegalStateException::class.java) { BackupFiles.copy(stalled, ByteArrayOutputStream()) }
        val failed = object : OutputStream() { override fun write(value: Int) { throw java.io.IOException() } }
        assertThrows(java.io.IOException::class.java) { BackupFiles.copy(ByteArrayInputStream(bytes), failed) }
    }

    @Test fun ciphertextReadbackDetectsTruncationAndSameLengthMutation() {
        val bytes = checkNotNull(javaClass.classLoader?.getResourceAsStream("backup-v1.tundra")).use { it.readBytes() }
        val expected = BackupFiles.digest(ByteArrayInputStream(bytes))
        assertEquals(bytes.size.toLong(), expected.first)
        assertArrayEquals(MessageDigest.getInstance("SHA-256").digest(bytes), expected.second)
        val truncated = BackupFiles.digest(ByteArrayInputStream(bytes.copyOf(bytes.size - 1)))
        assertNotEquals(expected.first, truncated.first)
        val changed = bytes.copyOf().also { it[100] = (it[100].toInt() xor 1).toByte() }
        val actual = BackupFiles.digest(ByteArrayInputStream(changed))
        assertEquals(expected.first, actual.first)
        assertFalse(MessageDigest.isEqual(expected.second, actual.second))
    }
}

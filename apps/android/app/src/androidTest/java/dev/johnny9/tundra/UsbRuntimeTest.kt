@file:OptIn(ExperimentalUnsignedTypes::class)
package dev.johnny9.tundra

import android.net.Uri
import androidx.test.platform.app.InstrumentationRegistry
import dev.johnny9.tundra.generated.*
import org.junit.Assert.*
import org.junit.Test

/** Public protocol transcripts exercise real Android FFI; they do not emulate qualified hardware. */
class UsbRuntimeTest {
    private fun answer(session: UsbConnection, payload: ByteArray): UsbInfo {
        val step = session.progress().step
        val response = payload + byteArrayOf(0x90.toByte(), 0)
        val bytes = byteArrayOf((response.size shr 8).toByte(), response.size.toByte()) + response
        var update = session.progress()
        for ((sequence, chunk) in bytes.toList().chunked(59).withIndex()) {
            val report = ByteArray(64)
            byteArrayOf(1, 1, 5, (sequence shr 8).toByte(), sequence.toByte()).copyInto(report)
            chunk.toByteArray().copyInto(report, 5)
            update = session.receive(step, report)
        }
        return update
    }
    @Test fun permissionFilterAcceptsOnlyItsNonceUriScheme() {
        val filter = usbPermissionFilter("test.USB_PERMISSION")
        assertTrue(filter.matchAction("test.USB_PERMISSION"))
        assertTrue(filter.matchData(null, "tundra-usb", Uri.parse("tundra-usb:public-test-nonce")) >= 0)
        assertTrue(filter.matchData(null, "https", Uri.parse("https://unused.invalid")) < 0)
        assertTrue(filter.matchData(null, null, null) < 0)
    }
    @Test fun publicAccountHandshakeCancellationAndSigningGateCrossFfi() {
        val descriptor = InstrumentationRegistry.getInstrumentation().context.assets.open("single-sig.txt")
            .bufferedReader().use { it.readText() }
        val fingerprint = Regex("\\[([a-f0-9]{8})/").find(descriptor)!!.groupValues[1]
        val xpub = Regex("tpub[A-Za-z0-9]+").find(descriptor)!!.value
        Tundra.open(":memory:").use { core ->
            val wallet = core.importWallet("Public USB fixture", descriptor, Chain.SIGNET)
            core.prepareUsb(wallet.id, UsbOperation.Inspect).use { session ->
                assertEquals(UsbState.WAITING, session.progress().state)
                assertArrayEquals(byteArrayOf(0xb0.toByte(), 1, 0, 0, 0), session.progress().packets.single().copyOfRange(7, 12))
                answer(session, byteArrayOf(1, 12) + "Bitcoin Test".toByteArray() + byteArrayOf(5) + "2.4.1".toByteArray() + byteArrayOf(1, 0))
                answer(session, fingerprint.chunked(2).map { it.toInt(16).toByte() }.toByteArray())
                val update = answer(session, xpub.toByteArray())
                assertEquals(UsbState.COMPLETE, update.state)
                assertEquals(fingerprint, update.fingerprint)
                assertEquals("2.4.1", update.appVersion)
            }
            core.prepareUsb(wallet.id, UsbOperation.Inspect).use { session ->
                assertEquals(UsbState.CANCELLED, session.cancel().state)
                try { answer(session, byteArrayOf()); fail("Late USB responses must be rejected") }
                catch (_: AppException.Operation) { }
            }
            try { core.prepareUsb(wallet.id, UsbOperation.SignDraft("missing-draft")); fail("Hardware signing requires device qualification") }
            catch (_: AppException.Operation) { }
            assertTrue(core.drafts(wallet.id).isEmpty())
            assertNull(core.wallets().single().totalSats)
        }
    }
}

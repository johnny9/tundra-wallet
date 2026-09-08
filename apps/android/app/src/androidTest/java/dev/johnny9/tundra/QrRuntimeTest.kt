@file:OptIn(ExperimentalUnsignedTypes::class)
package dev.johnny9.tundra

import android.util.Base64
import androidx.test.platform.app.InstrumentationRegistry
import com.google.zxing.BinaryBitmap
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader
import dev.johnny9.tundra.generated.*
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test

/** Independent ZXing decoding of the actual Rust matrix, on the Android ABI. */
internal fun decodeQrMatrix(frame: String): String {
    val image = renderQrFrame(frame)
    val side = image.side.toInt()
    val scale = 8
    val width = side * scale
    val pixels = IntArray(width * width) { index ->
        if (image.modules[(index / width / scale) * side + index % width / scale].toInt() == 0) -1 else 0xff000000.toInt()
    }
    return QRCodeReader().decode(BinaryBitmap(HybridBinarizer(RGBLuminanceSource(width, width, pixels)))).text
}

class QrRuntimeTest {
    private fun fixture(name: String) = InstrumentationRegistry.getInstrumentation().context.assets
        .open(name).bufferedReader().use { it.readText().trim() }

    @Test fun publicUrAndBbqrImagesCrossRealMobileFfi() {
        val ur = fixture("qr-registry-psbt.ur")
        QrScanner(QrPurpose.SignedPsbt).use { scanner ->
            val text = decodeQrMatrix(ur)
            assertEquals(ur.uppercase(), text)
            assertEquals(QrState.COMPLETE, scanner.receive(text).state)
            assertTrue(scanner.payload().copyOfRange(0, 5).contentEquals(byteArrayOf(112, 115, 98, 116, -1)))
        }
        val expected = Base64.decode(fixture("hwi-signed-wpkh.psbt"), Base64.DEFAULT)
        val vectors = JSONObject(fixture("qr-bbqr-vectors.json")).getJSONArray("vectors")
        for (i in 0 until vectors.length()) {
            val frames = vectors.getJSONObject(i).getJSONArray("frames")
            QrScanner(QrPurpose.SignedPsbt).use { scanner ->
                for (j in frames.length() - 1 downTo 0) {
                    val frame = frames.getString(j)
                    val text = decodeQrMatrix(frame)
                    assertEquals(frame, text)
                    val progress = scanner.receive(text)
                    if (progress.state != QrState.COMPLETE) assertEquals(progress, scanner.receive(text))
                }
                assertEquals(QrState.COMPLETE, scanner.progress().state)
                assertArrayEquals(expected, scanner.payload())
                assertEquals(QrState.CANCELLED, scanner.cancel().state)
                try { scanner.payload(); fail("Cancellation must discard the payload") }
                catch (_: AppException.Operation) { }
            }
        }
    }
    @Test fun descriptorQrRequiresSelectedNetworkAndExplicitNewSession() {
        val descriptor = fixture("single-sig.txt")
        val text = decodeQrMatrix(descriptor)
        QrScanner(QrPurpose.Descriptor(Chain.SIGNET)).use { scanner ->
            assertEquals(QrState.COMPLETE, scanner.receive(text).state)
            assertEquals(descriptor, scanner.payload().toString(Charsets.UTF_8))
        }
        QrScanner(QrPurpose.SignedPsbt).use { scanner ->
            try { scanner.receive(text); fail("A descriptor cannot be a signed response") }
            catch (_: AppException.Operation) { }
            assertEquals(QrState.FAILED, scanner.progress().state)
        }
    }
}

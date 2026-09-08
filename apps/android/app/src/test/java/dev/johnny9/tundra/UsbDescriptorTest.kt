package dev.johnny9.tundra

import org.junit.Assert.*
import org.junit.Test

class UsbDescriptorTest {
    // Test-only HID descriptor: one vendor application, 64-byte input/output, no report ID.
    private val descriptor = listOf(0x06, 0xa0, 0xff, 0x09, 1, 0xa1, 1,
        0x75, 8, 0x95, 64, 0x81, 8, 0x91, 8, 0xc0).map { it.toByte() }.toByteArray()
    @Test fun vendorReportsMustMatchTheWireLayout() {
        assertTrue(isLedgerHidDescriptor(descriptor))
        assertFalse(isLedgerHidDescriptor(descriptor.copyOf().also { it[1] = 1 })) // U2F/other usage.
        assertFalse(isLedgerHidDescriptor(descriptor.copyOf().also { it[10] = 63 })) // Short reports.
        assertFalse(isLedgerHidDescriptor(descriptor.copyOf().also { it[10] = 65 })) // Oversized reports.
        assertFalse(isLedgerHidDescriptor(descriptor.copyOfRange(0, 7) + byteArrayOf(0x85.toByte(), 1) + descriptor.copyOfRange(7, descriptor.size)))
        assertFalse(isLedgerHidDescriptor(descriptor + descriptor)) // Ambiguous applications.
    }
    @Test fun malformedItemsCannotSelectAnInterface() {
        for (size in 0 until descriptor.size) assertFalse(isLedgerHidDescriptor(descriptor.copyOf(size)))
        assertFalse(isLedgerHidDescriptor(byteArrayOf(0xfe.toByte(), 1, 0, 0)))
        assertFalse(isLedgerHidDescriptor(byteArrayOf(0xb4.toByte()) + descriptor)) // Global-stack underflow.
        assertFalse(isLedgerHidDescriptor(byteArrayOf(0xa4.toByte()) + descriptor)) // Unbalanced push.
        assertFalse(isLedgerHidDescriptor(ByteArray(1025)))
        assertFalse(isLedgerHidDescriptor(byteArrayOf(0xc0.toByte()) + descriptor))
    }
}

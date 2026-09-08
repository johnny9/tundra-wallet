package dev.johnny9.tundra

import dev.johnny9.tundra.generated.*
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

/** Calls the generated binding and real host library. Only disposable public fixtures are used. */
class NativeCoreTest {
    @get:Rule val temporary = TemporaryFolder()
    private val fixture get() = checkNotNull(javaClass.getResource("/single-sig.txt")).readText()

    @Test fun importAndReopenPreserveUnicodeUnknownBalanceAndReceiveIndex() {
        val path = temporary.newFile("wallet.sqlite").absolutePath
        val imported = Tundra.open(path).use { core ->
            val wallet = core.importWallet("Épargne 🧊", fixture, Chain.SIGNET)
            assertFalse(wallet.synced)
            assertNull(wallet.totalSats)
            assertNull(wallet.availableSats)
            val address = core.receiveAddress(wallet.id)
            assertFalse(address.hardwareVerified)
            core.setLabel(wallet.id, "addr", address.address, "家族の貯蓄 🧊")
            Triple(wallet.id, address, core.exportLabels(wallet.id))
        }
        Tundra.open(path).use { core ->
            val (id, first, labels) = imported
            val wallet = core.wallets().single()
            assertEquals(id, wallet.id)
            assertEquals("Épargne 🧊", wallet.name)
            assertFalse(wallet.synced)
            assertNull(wallet.totalSats)
            assertNull(wallet.availableSats)
            assertTrue(core.coins(id).isEmpty())
            assertTrue(core.activity(id).isEmpty())
            assertEquals(labels, core.exportLabels(id))
            val next = core.receiveAddress(id)
            assertEquals(first.index + 1u, next.index)
            assertNotEquals(first.address, next.address)
        }
    }

    @Test fun nativeErrorsAreTypedAndDoNotEchoInput() {
        Tundra.open(":memory:").use { core ->
            val sentinel = "private-input-must-not-appear-in-errors"
            val error = assertThrows(AppException.Operation::class.java) {
                core.previewImport(sentinel, Chain.SIGNET)
            }
            assertEquals(ErrorCode.INVALID_INPUT, error.code)
            assertFalse(error.detail.contains(sentinel))
            assertTrue(error.detail.isNotBlank())
        }
    }

    @Test fun unsignedAmountsCrossTheBridgeWithoutTruncation() {
        assertEquals(4_294_967_297uL, parseBtcAmount("42.94967297"))
        assertEquals("184467440737.09551615", formatBalance(ULong.MAX_VALUE))
        val error = assertThrows(AppException.Operation::class.java) {
            parseBtcAmount("0.000000001")
        }
        assertEquals(ErrorCode.INVALID_INPUT, error.code)
    }
}

@file:OptIn(ExperimentalUnsignedTypes::class)
package dev.johnny9.tundra

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import dev.johnny9.tundra.generated.BroadcastRequest
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test

class BroadcastRuntimeTest {
    @get:Rule val compose = createComposeRule()

    @Test fun explicitEndpointAndRetryConsentResetWhenEndpointChanges() {
        // Presentation-only request using the published multisig transaction ID; no IO.
        val initial = BroadcastRequest("public-wallet", "public-draft", "https://unused.invalid",
            "3dfe302dc91e27edb25f4713e511bcdf178854a3804d54f292c5961f14f9c843", 4_294_967_297uL, false, false)
        var submitted: BroadcastRequest? = null
        compose.setContent { MaterialTheme { BroadcastDialog(initial, {}, { submitted = it }) } }
        val confirm = compose.onNodeWithTag("confirmBroadcast")
        confirm.assertIsNotEnabled()
        compose.onNodeWithTag("broadcastConsent").performScrollTo().performClick()
        confirm.assertIsNotEnabled()
        compose.onNodeWithTag("broadcastRetryConsent").performScrollTo().performClick()
        confirm.assertIsEnabled()
        compose.onNodeWithTag("broadcastEndpoint").performScrollTo().performTextReplacement("http://127.0.0.1:3002")
        compose.onNodeWithTag("broadcastEndpoint").performImeAction()
        confirm.assertIsNotEnabled()
        compose.onNodeWithTag("broadcastConsent").assertIsOff()
        compose.onNodeWithTag("broadcastRetryConsent").assertIsOff()
        compose.onNodeWithTag("broadcastConsent").performScrollTo().performClick()
        compose.onNodeWithTag("broadcastRetryConsent").performScrollTo().performClick()
        confirm.performClick()
        compose.runOnIdle {
            assertEquals(initial.copy(endpoint = "http://127.0.0.1:3002", privacyConsent = true, retryAcknowledged = true), submitted)
        }
    }
}

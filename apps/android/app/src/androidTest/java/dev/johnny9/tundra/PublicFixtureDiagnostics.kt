package dev.johnny9.tundra

import android.graphics.Bitmap
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File

/** Test APK only, on the runner's disposable public wallets. Never dump UI trees. */
fun capturePublicFixtureScreenshot(name: String) {
    require(name.matches(Regex("[a-z-]{1,64}")))
    runCatching {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val root = File(checkNotNull(instrumentation.targetContext.getExternalFilesDir(null)), "public-test-diagnostics")
        check(root.isDirectory || root.mkdirs())
        val screenshot = checkNotNull(instrumentation.uiAutomation.takeScreenshot())
        try { File(root, "$name.png").outputStream().use { check(screenshot.compress(Bitmap.CompressFormat.PNG, 100, it)) } }
        finally { screenshot.recycle() }
    }
}

@file:OptIn(ExperimentalUnsignedTypes::class)
package dev.johnny9.tundra

import android.Manifest
import android.content.pm.PackageManager
import android.os.SystemClock
import android.util.Size
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size as DrawSize
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.google.zxing.BinaryBitmap
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.ReaderException
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader
import dev.johnny9.tundra.generated.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

/** Pixels stay on Android. Only a decoded QR string crosses the Rust boundary. */
private class CameraQrAnalyzer(private val frame: (String) -> Unit) : ImageAnalysis.Analyzer {
    private val reader = QRCodeReader()
    private var lastScan = 0L
    override fun analyze(image: ImageProxy) {
        try {
            val now = SystemClock.elapsedRealtime()
            if (now - lastScan < 180) return
            lastScan = now
            val width = image.width; val height = image.height
            if (width <= 0 || height <= 0 || width.toLong() * height > 2_073_600) return
            val plane = image.planes.firstOrNull() ?: return
            val buffer = plane.buffer
            val origin = buffer.position()
            val last = (height - 1L) * plane.rowStride + (width - 1L) * plane.pixelStride
            if (plane.pixelStride <= 0 || plane.rowStride <= 0 || last >= buffer.remaining()) return
            val pixels = ByteArray(width * height)
            for (y in 0 until height) for (x in 0 until width) {
                pixels[y * width + x] = buffer.get(origin + y * plane.rowStride + x * plane.pixelStride)
            }
            val source = PlanarYUVLuminanceSource(pixels, width, height, 0, 0, width, height, false)
            val text = reader.decode(BinaryBitmap(HybridBinarizer(source))).text
            if (text.length <= 4_296) frame(text)
        } catch (_: ReaderException) {
            // A video frame without a readable QR is normal. Never log its pixels/text.
        } finally { reader.reset(); image.close() }
    }
}

@Composable private fun CameraPreview(onFrame: (String) -> Unit, onFailure: () -> Unit) {
    val context = LocalContext.current
    val lifecycle = LocalLifecycleOwner.current
    val currentFrame by rememberUpdatedState(onFrame)
    val currentFailure by rememberUpdatedState(onFailure)
    val view = remember { PreviewView(context).apply { implementationMode = PreviewView.ImplementationMode.COMPATIBLE } }
    DisposableEffect(lifecycle, view) {
        val closed = AtomicBoolean(false)
        val executor = Executors.newSingleThreadExecutor()
        val main = ContextCompat.getMainExecutor(context)
        val future = ProcessCameraProvider.getInstance(context)
        var provider: ProcessCameraProvider? = null
        val preview = Preview.Builder().build().also { it.setSurfaceProvider(view.surfaceProvider) }
        val analysis = ImageAnalysis.Builder().setTargetResolution(Size(1280, 720))
            .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST).build()
        analysis.setAnalyzer(executor, CameraQrAnalyzer { text ->
            main.execute { if (!closed.get()) currentFrame(text) }
        })
        future.addListener({
            if (!closed.get()) {
                try {
                    provider = future.get()
                    provider!!.bindToLifecycle(lifecycle, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
                } catch (_: Exception) { currentFailure() }
            }
        }, main)
        onDispose {
            closed.set(true); analysis.clearAnalyzer(); provider?.unbind(preview, analysis); executor.shutdownNow()
        }
    }
    AndroidView(factory = { view }, modifier = Modifier.fillMaxWidth().aspectRatio(1f))
}

@Composable fun QrScanDialog(purpose: QrPurpose, onPayload: (ByteArray) -> Unit, onClose: () -> Unit) {
    val context = LocalContext.current
    val lifecycle = LocalLifecycleOwner.current
    val scope = rememberCoroutineScope()
    var allowed by remember { mutableStateOf(ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) }
    var scanner by remember { mutableStateOf(QrScanner(purpose)) }
    var info by remember { mutableStateOf<QrInfo?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var processing by remember { mutableStateOf(false) }
    val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        allowed = granted
        if (granted) { scanner = QrScanner(purpose); info = null; error = null }
    }
    DisposableEffect(scanner, lifecycle) {
        val session = scanner
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_STOP) { info = session.cancel(); error = "Scan cancelled when the app left the screen. Start a new scan to continue." }
        }
        lifecycle.lifecycle.addObserver(observer)
        onDispose { lifecycle.lifecycle.removeObserver(observer); session.cancel(); session.close() }
    }
    LaunchedEffect(scanner) {
        val session = scanner
        while (isActive) {
            delay(250)
            val progress = try { withContext(Dispatchers.Default) { session.progress() } }
            catch (_: IllegalStateException) { break }
            info = progress
            if (progress.state == QrState.FAILED) { error = "Scan expired. Start a new scan to continue."; break }
            if (progress.state != QrState.SCANNING) break
        }
    }
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Surface(Modifier.fillMaxSize()) {
            Column(Modifier.safeDrawingPadding().padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Text(if (purpose is QrPurpose.SignedPsbt) "Scan signed PSBT" else "Scan public descriptor", style = MaterialTheme.typography.headlineSmall)
                if (!allowed) {
                    Text("Allow camera access to scan. You can also import a file from the previous screen.")
                    Button(onClick = { permission.launch(Manifest.permission.CAMERA) }) { Text("Enable camera") }
                } else if (error == null && (info == null || info?.state == QrState.SCANNING)) {
                    CameraPreview(onFrame = { text ->
                        if (!processing) {
                            processing = true
                            val session = scanner
                            scope.launch {
                                try {
                                    val progress = withContext(Dispatchers.Default) { session.receive(text) }
                                    if (scanner === session) {
                                        info = progress
                                        if (progress.state == QrState.COMPLETE) {
                                            val payload = withContext(Dispatchers.Default) { session.payload() }
                                            onPayload(payload)
                                        }
                                    }
                                } catch (failure: AppException.Operation) { if (scanner === session) error = failure.detail }
                                catch (_: IllegalStateException) { if (scanner === session) error = "Scan closed. Start a new scan to continue." }
                                finally { processing = false }
                            }
                        }
                    }, onFailure = { error = "Camera unavailable. Close this scan and import a file."; info = scanner.cancel() })
                    Text("${info?.resolvedFragments ?: 0u} / ${info?.totalFragments?.toString() ?: "?"} fragments")
                }
                error?.let {
                    Text(it, color = MaterialTheme.colorScheme.error)
                    Button(onClick = { scanner = QrScanner(purpose); info = null; error = null }) { Text("Start a new scan") }
                }
                TextButton(onClick = onClose) { Text("Close scan") }
            }
        }
    }
}

@Composable fun QrDisplayDialog(frames: List<String>, onClose: () -> Unit) {
    val lifecycle = LocalLifecycleOwner.current
    val close by rememberUpdatedState(onClose)
    DisposableEffect(lifecycle) {
        val observer = LifecycleEventObserver { _, event -> if (event == Lifecycle.Event.ON_STOP) close() }
        lifecycle.lifecycle.addObserver(observer)
        onDispose { lifecycle.lifecycle.removeObserver(observer) }
    }
    var index by remember(frames) { mutableIntStateOf(0) }
    var paused by remember { mutableStateOf(false) }
    var matrix by remember { mutableStateOf<QrImage?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(frames, index) {
        if (frames.isEmpty()) return@LaunchedEffect
        try { matrix = withContext(Dispatchers.Default) { renderQrFrame(frames[index]) } }
        catch (failure: AppException.Operation) { error = failure.detail; paused = true }
    }
    LaunchedEffect(frames, paused) {
        while (isActive && !paused && frames.size > 1) { delay(350); index = (index + 1) % frames.size }
    }
    Dialog(onDismissRequest = onClose) {
        Surface(shape = MaterialTheme.shapes.large) {
            Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Text("PSBT for your hardware", style = MaterialTheme.typography.titleLarge)
                matrix?.let { image ->
                    Canvas(Modifier.fillMaxWidth().aspectRatio(1f).background(Color.White).semantics { contentDescription = "PSBT QR frame" }) {
                        val side = image.side.toInt(); val unit = size.width / side
                        for (y in 0 until side) for (x in 0 until side) if (image.modules[y * side + x].toInt() != 0) {
                            drawRect(Color.Black, Offset(x * unit, y * unit), DrawSize(unit, unit))
                        }
                    }
                }
                error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                Text("Frame ${index + 1} of ${frames.size}. Compare the transaction on your hardware.")
                if (frames.size > 1) TextButton(onClick = { paused = !paused }) { Text(if (paused) "Resume" else "Pause") }
                TextButton(onClick = onClose) { Text("Close QR") }
            }
        }
    }
}

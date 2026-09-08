@file:OptIn(ExperimentalUnsignedTypes::class)
package dev.johnny9.tundra

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.*
import android.net.Uri
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.content.ContextCompat
import androidx.core.content.IntentCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import dev.johnny9.tundra.generated.*
import kotlinx.coroutines.*
import java.io.IOException
import java.nio.ByteBuffer
import java.util.concurrent.TimeoutException
import java.util.UUID

/** Parse bounded HID items; only the Ledger vendor application usage page is accepted. */
internal fun isLedgerHidDescriptor(bytes: ByteArray): Boolean {
    if (bytes.isEmpty() || bytes.size > 1024) return false
    var offset = 0; var page = -1; var depth = 0; var ledger = false
    var reportSize = 0; var reportCount = 0; var inputBits = 0; var outputBits = 0
    val globals = ArrayDeque<Triple<Int, Int, Int>>()
    while (offset < bytes.size) {
        val prefix = bytes[offset++].toInt() and 255
        if (prefix == 254) return false // No long items needed by this restricted Ledger transport.
        val size = if (prefix and 3 == 3) 4 else prefix and 3
        if (offset + size > bytes.size) return false
        var value = 0L
        repeat(size) { value = value or ((bytes[offset + it].toLong() and 255) shl (8 * it)) }
        offset += size
        val type = (prefix shr 2) and 3; val tag = prefix shr 4
        when {
            type == 1 && tag == 0 -> { if (value > 65535) return false; page = value.toInt() }
            type == 1 && tag == 7 -> { if (value > 64) return false; reportSize = value.toInt() }
            type == 1 && tag == 8 -> return false // Report IDs would change the 64-byte wire layout.
            type == 1 && tag == 9 -> { if (value > 512) return false; reportCount = value.toInt() }
            type == 1 && tag == 10 -> { if (size != 0 || globals.size >= 8) return false; globals.addLast(Triple(page, reportSize, reportCount)) }
            type == 1 && tag == 11 -> {
                if (size != 0 || globals.isEmpty()) return false
                val saved = globals.removeLast(); page = saved.first; reportSize = saved.second; reportCount = saved.third
            }
            type == 0 && (tag == 8 || tag == 9) -> {
                if (depth == 0 || reportSize == 0 || reportCount == 0) return false
                val bits = reportSize * reportCount
                if (tag == 8) inputBits += bits else outputBits += bits
                if (inputBits > 512 || outputBits > 512) return false
            }
            type == 0 && tag == 10 -> {
                if (depth == 0) { if (ledger || value != 1L || page != 0xffa0) return false; ledger = true }
                if (++depth > 8) return false
            }
            type == 0 && tag == 12 -> { if (size != 0 || --depth < 0) return false }
        }
    }
    return ledger && depth == 0 && globals.isEmpty() && inputBits == 512 && outputBits == 512
}

/** USB handles are owned by one IO coroutine, including cancellation cleanup. */
private class LedgerUsbLink(
    private val connection: UsbDeviceConnection,
    private val intf: UsbInterface,
    private val input: UsbEndpoint,
    private val output: UsbEndpoint
) : AutoCloseable {
    private var closed = false
    private var reading: UsbRequest? = null
    private var buffer: ByteBuffer? = null
    fun write(report: ByteArray) {
        if (closed || report.size != 64 || reading != null) throw IOException()
        val request = UsbRequest()
        try {
            if (!request.initialize(connection, output)) throw IOException()
            val bytes = ByteBuffer.wrap(report)
            if (!request.queue(bytes) || connection.requestWait(1500) !== request || bytes.position() != 64) throw IOException()
        } finally { request.cancel(); request.close() }
    }
    fun read(): ByteArray? {
        if (closed) throw IOException()
        if (reading == null) {
            val request = UsbRequest(); val bytes = ByteBuffer.allocate(64)
            if (!request.initialize(connection, input) || !request.queue(bytes)) { request.close(); throw IOException() }
            reading = request; buffer = bytes
        }
        // Keep the same queued read across timeouts; cancelling/requeuing can lose a report.
        val result = try { connection.requestWait(500) } catch (_: TimeoutException) { return null }
        val request = reading ?: throw IOException()
        val bytes = buffer ?: throw IOException()
        if (result !== request || bytes.position() != 64) throw IOException()
        val report = bytes.array().copyOf()
        reading = null; buffer = null; request.close()
        return report
    }
    override fun close() {
        if (closed) return
        closed = true
        try { reading?.let { try { it.cancel() } finally { it.close() } } }
        finally {
            reading = null; buffer = null
            try { connection.releaseInterface(intf) } finally { connection.close() }
        }
    }
    companion object {
        fun candidates(manager: UsbManager): List<UsbDevice> = manager.deviceList.values.filter { device ->
            device.vendorId == 0x2c97 && (0 until device.interfaceCount).any { device.getInterface(it).interfaceClass == UsbConstants.USB_CLASS_HID }
        }.sortedBy { it.deviceId }
        fun open(manager: UsbManager, device: UsbDevice): LedgerUsbLink {
            if (!manager.hasPermission(device) || manager.deviceList[device.deviceName]?.deviceId != device.deviceId) throw IOException()
            val connection = manager.openDevice(device) ?: throw IOException()
            try {
                for (index in 0 until device.interfaceCount) {
                    val intf = device.getInterface(index)
                    if (intf.interfaceClass != UsbConstants.USB_CLASS_HID) continue
                    val endpoints = (0 until intf.endpointCount).map(intf::getEndpoint)
                        .filter { it.type == UsbConstants.USB_ENDPOINT_XFER_INT && it.maxPacketSize == 64 }
                    val input = endpoints.singleOrNull { it.direction == UsbConstants.USB_DIR_IN } ?: continue
                    val output = endpoints.singleOrNull { it.direction == UsbConstants.USB_DIR_OUT } ?: continue
                    val descriptor = ByteArray(1024)
                    val length = connection.controlTransfer(UsbConstants.USB_DIR_IN or 1, 6, 0x2200, intf.id, descriptor, descriptor.size, 1500)
                    if (length <= 0 || !isLedgerHidDescriptor(descriptor.copyOf(length))) continue
                    if (!connection.claimInterface(intf, true)) continue
                    return LedgerUsbLink(connection, intf, input, output)
                }
                throw IOException()
            } catch (failure: Exception) { connection.close(); throw failure }
        }
    }
}

internal fun usbPermissionFilter(action: String) = IntentFilter(action).apply { addDataScheme("tundra-usb") }

private data class PendingUsbPermission(
    val device: UsbDevice, val operation: UsbOperation, val token: Uri, val permission: PendingIntent
)

@Composable fun UsbHardwareDialog(vm: WalletViewModel, walletId: String, receiveIndex: UInt?, onClose: () -> Unit) {
    val context = LocalContext.current
    val lifecycle = LocalLifecycleOwner.current
    val scope = rememberCoroutineScope()
    val manager = remember { context.getSystemService(UsbManager::class.java) }
    if (manager == null) {
        AlertDialog(onDismissRequest = onClose, title = { Text("USB hardware") },
            text = { Text("USB host is not available on this Android runtime.") },
            confirmButton = { TextButton(onClick = onClose) { Text("Close USB") } })
        return
    }
    val permissionAction = remember { "${context.packageName}.USB_PERMISSION.${UUID.randomUUID()}" }
    var devices by remember { mutableStateOf(LedgerUsbLink.candidates(manager)) }
    var selected by remember { mutableStateOf(devices.firstOrNull()?.deviceId) }
    var pending by remember { mutableStateOf<PendingUsbPermission?>(null) }
    var job by remember { mutableStateOf<Job?>(null) }
    var activeDevice by remember { mutableStateOf<Int?>(null) }
    var info by remember { mutableStateOf<UsbInfo?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var operation by remember { mutableStateOf<UsbOperation?>(null) }
    var generation by remember { mutableIntStateOf(0) }
    val stop: (String?) -> Unit = { message ->
        generation++; pending?.permission?.cancel(); pending = null; job?.cancel()
        // Keep actions disabled until the IO coroutine closes its handles. Each wait is
        // bounded, so cancellation never needs a concurrent close of a UsbRequest.
        info = null
        if (message != null) error = message
    }
    val launch: (UsbDevice, UsbOperation) -> Unit = { device, requested ->
        if (job == null) {
            error = null; info = null; operation = requested; activeDevice = device.deviceId
            val token = ++generation
            job = scope.launch {
                try {
                    withContext(Dispatchers.IO) {
                        var current: UsbConnection? = null
                        var transport: LedgerUsbLink? = null
                        try {
                            // Acquire and close inside the same dispatcher block. Cancellation
                            // at the dispatcher return boundary cannot leak an acquired handle.
                            val active = vm.prepareUsb(walletId, requested).also { current = it }
                            ensureActive()
                            val io = LedgerUsbLink.open(manager, device).also { transport = it }
                            ensureActive()
                            var update = active.progress()
                            while (true) {
                                ensureActive()
                                withContext(Dispatchers.Main) {
                                    if (token != generation) throw CancellationException()
                                    info = update
                                }
                                when (update.state) {
                                    UsbState.COMPLETE -> break
                                    UsbState.CANCELLED -> throw CancellationException()
                                    UsbState.FAILED -> throw IOException()
                                    UsbState.WAITING -> { }
                                }
                                for (packet in update.packets) { ensureActive(); io.write(packet) }
                                val report = io.read()
                                ensureActive()
                                update = if (report == null) active.progress().copy(packets = emptyList())
                                    else active.receive(update.step, report)
                            }
                        } finally {
                            try { transport?.close() }
                            finally { current?.let { try { it.cancel() } finally { it.close() } } }
                        }
                    }
                } catch (failure: CancellationException) { throw failure }
                catch (failure: Exception) {
                    if (token == generation) error = if (failure is AppException.Operation) failure.detail else "USB operation stopped. Check permission, cable and the Bitcoin Test app, then reconnect."
                } finally { activeDevice = null; job = null }
            }
        }
    }
    val latestStop by rememberUpdatedState(stop)
    val latestLaunch by rememberUpdatedState(launch)
    DisposableEffect(manager, lifecycle) {
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                if (intent.action == UsbManager.ACTION_USB_DEVICE_DETACHED) {
                    val device = IntentCompat.getParcelableExtra(intent, UsbManager.EXTRA_DEVICE, UsbDevice::class.java)
                    if (device != null && (device.deviceId == activeDevice || device.deviceId == pending?.device?.deviceId))
                        latestStop("USB device detached. Reconnect and start the operation again.")
                    devices = LedgerUsbLink.candidates(manager)
                    if (devices.none { it.deviceId == selected }) selected = devices.firstOrNull()?.deviceId
                    return
                }
                if (intent.action != permissionAction) return
                val target = pending ?: return
                val returned = IntentCompat.getParcelableExtra(intent, UsbManager.EXTRA_DEVICE, UsbDevice::class.java) ?: return
                if (intent.data != target.token || returned.deviceId != target.device.deviceId || returned.deviceName != target.device.deviceName) return
                pending = null; target.permission.cancel()
                if (!lifecycle.lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) return
                if (intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false) && manager.hasPermission(target.device)) latestLaunch(target.device, target.operation)
                else error = "USB permission was denied. Select the device and try again."
            }
        }
        // Permission replies carry a nonce URI; detach broadcasts carry no URI. They need
        // separate filters, otherwise Android's data matching silently drops one of them.
        ContextCompat.registerReceiver(context, receiver, usbPermissionFilter(permissionAction), ContextCompat.RECEIVER_NOT_EXPORTED)
        ContextCompat.registerReceiver(context, receiver, IntentFilter(UsbManager.ACTION_USB_DEVICE_DETACHED), ContextCompat.RECEIVER_NOT_EXPORTED)
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_STOP && (job != null || pending != null))
                latestStop("USB operation cancelled when the app left the screen.")
        }
        lifecycle.lifecycle.addObserver(observer)
        onDispose { context.unregisterReceiver(receiver); lifecycle.lifecycle.removeObserver(observer); latestStop(null) }
    }
    val request: (UsbOperation) -> Unit = { requested ->
        val device = devices.firstOrNull { it.deviceId == selected }
        if (device == null) error = "Select a connected Ledger USB device."
        else if (manager.hasPermission(device)) launch(device, requested)
        else {
            error = null
            val token = Uri.parse("tundra-usb:${UUID.randomUUID()}")
            val intent = Intent(permissionAction).setPackage(context.packageName).setData(token)
            val permission = PendingIntent.getBroadcast(context, device.deviceId, intent, PendingIntent.FLAG_CANCEL_CURRENT or PendingIntent.FLAG_MUTABLE)
            pending = PendingUsbPermission(device, requested, token, permission)
            try { manager.requestPermission(device, permission) }
            catch (_: Exception) { permission.cancel(); pending = null; error = "USB permission request failed. Reconnect and try again." }
        }
    }
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Surface(Modifier.fillMaxSize()) {
            Column(Modifier.safeDrawingPadding().padding(24.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Text("USB hardware", style = MaterialTheme.typography.headlineSmall)
                Text("Development Ledger HID transport. No model or firmware is qualified yet. Open Bitcoin Test on your hardware. Signing remains unavailable.")
                if (devices.isEmpty()) Text("No Ledger HID device connected. Use a USB data cable and refresh.")
                for (device in devices) FilterChip(selected = device.deviceId == selected, onClick = { selected = device.deviceId }, label = { Text("Ledger USB device ${device.deviceId}") }, enabled = job == null && pending == null)
                TextButton(onClick = { devices = LedgerUsbLink.candidates(manager); selected = devices.firstOrNull()?.deviceId }, enabled = job == null && pending == null) { Text("Refresh devices") }
                Button(onClick = { request(UsbOperation.Inspect) }, enabled = selected != null && job == null && pending == null) { Text("Check public account") }
                OutlinedButton(onClick = { request(UsbOperation.RegisterPolicy) }, enabled = selected != null && job == null && pending == null) { Text("Register wallet policy") }
                if (receiveIndex != null) OutlinedButton(onClick = { request(UsbOperation.VerifyReceive(receiveIndex)) }, enabled = selected != null && job == null && pending == null) { Text("Compare receive address") }
                Text("Compare the policy and address on the hardware's own screen. Wallet labels are not sent to it.", style = MaterialTheme.typography.bodySmall)
                info?.let { value ->
                    value.appVersion?.let { Text("Bitcoin Test $it") }
                    value.fingerprint?.let { Text("Wallet fingerprint $it") }
                    if (value.state == UsbState.COMPLETE) Text(when (operation) {
                        is UsbOperation.RegisterPolicy -> "Policy registration saved for this wallet and public account."
                        is UsbOperation.VerifyReceive -> "Device response matches the issued receive address. Physical qualification remains open."
                        else -> "The device's public account matches this wallet."
                    })
                    else if (job != null) Text("Waiting for hardware · exchange ${value.step}")
                }
                if (pending != null) Text("Waiting for USB permission")
                error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                if (job != null || pending != null) TextButton(onClick = { stop("USB operation cancelled. Drafts are preserved.") }) { Text("Cancel USB operation") }
                TextButton(onClick = onClose) { Text("Close USB") }
            }
        }
    }
}

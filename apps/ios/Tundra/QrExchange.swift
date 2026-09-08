import AVFoundation
import SwiftUI
import UIKit

/// Camera frames stay native; only bounded decoded strings enter the Rust decoder.
private final class QrCamera: NSObject, AVCaptureMetadataOutputObjectsDelegate {
    let session = AVCaptureSession()
    private let queue = DispatchQueue(label: "dev.johnny9.tundra.qr-camera")
    private var configured = false
    private var lastFrame = Date.distantPast
    var frame: ((String) -> Void)?
    var failure: (() -> Void)?

    func start() {
        queue.async { [self] in
            do {
                if !configured {
                    guard let camera = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back) else {
                        DispatchQueue.main.async { self.failure?() }; return
                    }
                    let input = try AVCaptureDeviceInput(device: camera)
                    let output = AVCaptureMetadataOutput()
                    session.beginConfiguration()
                    defer { session.commitConfiguration() }
                    session.sessionPreset = .hd1280x720
                    guard session.canAddInput(input), session.canAddOutput(output) else {
                        DispatchQueue.main.async { self.failure?() }; return
                    }
                    session.addInput(input); session.addOutput(output)
                    output.setMetadataObjectsDelegate(self, queue: queue)
                    guard output.availableMetadataObjectTypes.contains(.qr) else {
                        session.removeInput(input); session.removeOutput(output)
                        DispatchQueue.main.async { self.failure?() }; return
                    }
                    output.metadataObjectTypes = [.qr]
                    configured = true
                }
                if !session.isRunning { session.startRunning() }
            } catch { DispatchQueue.main.async { self.failure?() } }
        }
    }
    func stop() { queue.async { [self] in if session.isRunning { session.stopRunning() } } }
    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput metadataObjects: [AVMetadataObject], from connection: AVCaptureConnection) {
        guard Date().timeIntervalSince(lastFrame) >= 0.18 else { return }
        guard let text = metadataObjects.compactMap({ ($0 as? AVMetadataMachineReadableCodeObject)?.stringValue }).first,
              text.utf8.count <= 4_296 else { return }
        lastFrame = Date()
        DispatchQueue.main.async { self.frame?(text) }
    }
}

private final class QrPreviewSurface: UIView {
    override class var layerClass: AnyClass { AVCaptureVideoPreviewLayer.self }
    var preview: AVCaptureVideoPreviewLayer { layer as! AVCaptureVideoPreviewLayer }
}
private struct QrCameraPreview: UIViewRepresentable {
    let session: AVCaptureSession
    func makeUIView(context: Context) -> QrPreviewSurface {
        let view = QrPreviewSurface(); view.preview.session = session; view.preview.videoGravity = .resizeAspectFill
        return view
    }
    func updateUIView(_ view: QrPreviewSurface, context: Context) { }
    static func dismantleUIView(_ view: QrPreviewSurface, coordinator: ()) { view.preview.session = nil }
}

private actor QrSession {
    private let scanner: QrScanner
    init(_ purpose: QrPurpose) { scanner = QrScanner(purpose: purpose) }
    func receive(_ text: String) throws -> QrInfo { try scanner.receive(frame: text) }
    func progress() -> QrInfo { scanner.progress() }
    func payload() throws -> Data { try scanner.payload() }
    func cancel() { _ = scanner.cancel() }
}

@MainActor private final class QrScanModel: ObservableObject {
    let camera = QrCamera()
    @Published var allowed = AVCaptureDevice.authorizationStatus(for: .video) == .authorized
    @Published var info: QrInfo?
    @Published var error: String?
    private var session: QrSession?
    private var generation = UUID()
    private var visible = false
    private var processing = false
    private var polling: Task<Void, Never>?
    private var purpose: QrPurpose = .signedPsbt
    private var completion: ((Data) -> Void)?

    func show(_ purpose: QrPurpose, completion: @escaping (Data) -> Void) {
        visible = true; self.purpose = purpose; self.completion = completion
        camera.frame = { [weak self] in self?.receive($0) }
        camera.failure = { [weak self] in self?.stop("Camera unavailable. Close this scan and import a file.") }
        if allowed { restart() }
    }
    func enableCamera() {
        AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
            Task { @MainActor in
                guard let self, self.visible else { return }
                self.allowed = granted
                if granted { self.restart() }
                else { self.error = "Camera permission is disabled. You can allow it in Settings or import a file." }
            }
        }
    }
    func restart() {
        guard visible, allowed else { return }
        stop(nil); error = nil; info = nil
        let current = QrSession(purpose); session = current
        let token = generation
        camera.start()
        polling = Task { [weak self] in
            while !Task.isCancelled {
                do { try await Task.sleep(for: .milliseconds(250)) } catch { return }
                let progress = await current.progress()
                guard let self, self.generation == token else { return }
                self.info = progress
                if progress.state == .failed { self.stop("Scan expired. Start a new scan to continue."); return }
                if progress.state != .scanning { return }
            }
        }
    }
    private func receive(_ text: String) {
        guard visible, !processing, let current = session else { return }
        processing = true
        let token = generation
        Task {
            defer { if generation == token { processing = false } }
            do {
                let progress = try await current.receive(text)
                guard generation == token else { return }
                info = progress
                if progress.state == .complete {
                    let data = try await current.payload()
                    guard generation == token else { return }
                    stop(nil); completion?(data)
                }
            } catch AppError.Operation(_, let detail) { if generation == token { stop(detail) } }
            catch { if generation == token { stop("QR could not be read. Start a new scan or import a file.") } }
        }
    }
    func stop(_ message: String?) {
        generation = UUID(); processing = false
        polling?.cancel(); polling = nil
        if let session { Task { await session.cancel() } }
        session = nil; camera.stop()
        if let message { error = message }
    }
    func hide() { visible = false; stop(nil); camera.frame = nil; camera.failure = nil; completion = nil }
}

struct QrScanView: View {
    let purpose: QrPurpose
    var onPayload: (Data) -> Void
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @StateObject private var scan = QrScanModel()
    var body: some View {
        NavigationStack {
            VStack(spacing: 20) {
                if !scan.allowed {
                    Text("Allow camera access to scan. You can also import a file from the previous screen.")
                    Button("Enable camera") { scan.enableCamera() }
                } else if scan.error == nil {
                    QrCameraPreview(session: scan.camera.session).aspectRatio(1, contentMode: .fit).clipped()
                    Text("\(scan.info?.resolvedFragments ?? 0) / \(scan.info?.totalFragments.map(String.init) ?? "?") fragments")
                }
                if let error = scan.error {
                    Text(error).foregroundStyle(.red)
                    if scan.allowed { Button("Start a new scan") { scan.restart() } }
                }
                Spacer()
            }.padding(24)
            .navigationTitle(purpose == .signedPsbt ? "Scan signed PSBT" : "Scan public descriptor")
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close scan") { scan.hide(); dismiss() } } }
        }
        .onAppear { scan.show(purpose, completion: onPayload) }
        .onDisappear { scan.hide() }
        .onChange(of: scenePhase) { _, phase in
            if phase != .active { scan.stop("Scan cancelled when the app left the screen. Start a new scan to continue.") }
        }
    }
}

/// The Rust matrix includes its quiet zone. Use the same native image in display and Vision tests.
func qrBitmap(_ matrix: QrImage) -> CGImage? {
    let side = Int(matrix.side)
    guard side > 0, side <= 512, matrix.modules.count == side * side else { return nil }
    let pixels = Data(matrix.modules.map { $0 == 0 ? UInt8(255) : UInt8(0) })
    guard let provider = CGDataProvider(data: pixels as CFData) else { return nil }
    return CGImage(width: side, height: side, bitsPerComponent: 8, bitsPerPixel: 8, bytesPerRow: side,
                   space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGBitmapInfo(rawValue: 0),
                   provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
}

struct QrDisplayView: View {
    let frames: [String]
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @State private var index = 0
    @State private var paused = false
    @State private var bitmap: CGImage?
    @State private var error: String?
    var body: some View {
        NavigationStack {
            VStack(spacing: 20) {
                if let bitmap {
                    Image(decorative: bitmap, scale: 1).interpolation(.none).resizable().scaledToFit()
                        .accessibilityLabel("PSBT QR frame")
                }
                if let error { Text(error).foregroundStyle(.red) }
                Text("Frame \(index + 1) of \(frames.count). Compare the transaction on your hardware.")
                if frames.count > 1 { Button(paused ? "Resume" : "Pause") { paused.toggle() } }
                Spacer()
            }.padding(24)
            .navigationTitle("PSBT for your hardware")
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close QR") { dismiss() } } }
        }
        .task(id: index) {
            guard frames.indices.contains(index) else { return }
            let frame = frames[index]
            do {
                let matrix = try await Task.detached(priority: .userInitiated) { try renderQrFrame(frame: frame) }.value
                try Task.checkCancellation()
                bitmap = qrBitmap(matrix)
            } catch is CancellationError { }
            catch { error = "QR could not be displayed. Close this screen and export a file."; paused = true }
        }
        .task(id: paused) {
            while !paused && frames.count > 1 {
                do { try await Task.sleep(for: .milliseconds(350)) } catch { return }
                index = (index + 1) % frames.count
            }
        }
        .onChange(of: scenePhase) { _, phase in if phase != .active { dismiss() } }
    }
}

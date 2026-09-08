import SwiftUI
import UniformTypeIdentifiers

struct LabelsView: View {
    @ObservedObject var model: WalletModel
    @Environment(\.dismiss) private var dismiss
    @State private var importing = false
    var body: some View {
        NavigationStack {
            Form {
                Text("Label exports are unencrypted, privacy-sensitive wallet metadata. Origin-tagged records must match this wallet's policy and key origins; only known references are applied.")
                Button("Import labels") { importing = true }.disabled(model.busy)
                Button("Export labels") { model.exportLabels() }.disabled(model.busy)
                if let preview = model.labelPreview {
                    Text("\(preview.matched) matched · \(preview.changed) changed · \(preview.skipped) skipped")
                    Text("Freezes may change. Draft reservations remain independent.")
                    Button("Apply labels") { model.applyLabels() }.disabled(model.busy)
                    Button("Cancel import") { model.cancelLabels() }.disabled(model.busy)
                }
                if let error = model.error { Text(error).foregroundStyle(.red) }
            }
            .navigationTitle("Labels")
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { model.cancelLabels(); dismiss() }.disabled(model.busy) } }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.plainText, .json, .data]) { result in
            if case .success(let url) = result { model.inspectLabels(url) }
        }
        .fileExporter(isPresented: Binding(get: { model.exportedLabels != nil }, set: { if !$0 { model.exportedLabels = nil } }),
            document: PublicTextExport(model.exportedLabels ?? ""), contentType: .plainText, defaultFilename: "tundra-labels.jsonl") { result in
                if case .failure = result { model.error = "Label export could not be saved." }
                model.exportedLabels = nil
            }
        .interactiveDismissDisabled(model.busy)
    }
}

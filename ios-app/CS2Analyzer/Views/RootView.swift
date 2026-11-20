import SwiftUI
import UniformTypeIdentifiers

/// Слева список матчей, справа сводка матча (этап I2) или повтор раунда (I3-I4).
struct RootView: View {
    @State private var library = MatchLibrary()
    @State private var settings = AppSettings()
    @State private var importCoordinator = ImportCoordinator()
    @State private var selection: String?
    @State private var columnVisibility: NavigationSplitViewVisibility = .all
    @State private var isImporterPresented = false
    @State private var isSettingsPresented = false

    private static let demoContentType = UTType(filenameExtension: "dem") ?? .data

    var body: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            MatchListView(
                items: library.items,
                loadErrors: library.loadErrors,
                selection: $selection,
                onImportTapped: { isImporterPresented = true },
                onSettingsTapped: { isSettingsPresented = true }
            )
        } detail: {
            if let selection, let match = library.match(for: selection) {
                MatchDetailView(match: match)
            } else {
                ContentUnavailableView(
                    "Выберите матч",
                    systemImage: "scope",
                    description: Text("Здесь появятся сводка и повтор раунда.")
                )
            }
        }
        .fileImporter(
            isPresented: $isImporterPresented,
            allowedContentTypes: [Self.demoContentType],
            allowsMultipleSelection: false
        ) { result in
            switch result {
            case .success(let urls):
                guard let url = urls.first else { return }
                Task {
                    if let summary = await importCoordinator.importDemo(url, into: library) {
                        selection = summary.id
                    }
                }
            case .failure(let error):
                importCoordinator.reportPickerFailure(error)
            }
        }
        .sheet(isPresented: importSheetBinding) {
            ImportProgressView(state: importCoordinator.state, onDismissError: importCoordinator.dismissError)
        }
        .sheet(isPresented: $isSettingsPresented) {
            SettingsView(library: library, settings: settings)
        }
    }

    private var importSheetBinding: Binding<Bool> {
        Binding(
            get: { importCoordinator.state != .idle },
            set: { isPresented in
                if !isPresented { importCoordinator.dismissError() }
            }
        )
    }
}

#Preview {
    RootView()
}

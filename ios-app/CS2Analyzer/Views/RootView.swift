import SwiftUI

/// Слева список матчей, справа сводка матча (этап I2) или повтор раунда (I3-I4).
struct RootView: View {
    @State private var library = MatchLibrary()
    @State private var selection: String?
    @State private var columnVisibility: NavigationSplitViewVisibility = .all
    @State private var settings = ServerSettings()
    @State private var showSettings = false

    var body: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            MatchListView(items: library.items, loadErrors: library.loadErrors, selection: $selection)
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button {
                            showSettings = true
                        } label: {
                            Image(systemName: "gearshape")
                        }
                        .accessibilityIdentifier("open-settings")
                    }
                }
                .sheet(isPresented: $showSettings) {
                    SettingsView(settings: settings)
                }
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
    }
}

#Preview {
    RootView()
}

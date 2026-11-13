import SwiftUI

/// Слева список матчей, справа сводка матча (этап I2) или повтор раунда (I3-I4).
struct RootView: View {
    @State private var library = MatchLibrary()
    @State private var selection: String?
    @State private var columnVisibility: NavigationSplitViewVisibility = .all

    var body: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            MatchListView(items: library.items, loadErrors: library.loadErrors, selection: $selection)
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

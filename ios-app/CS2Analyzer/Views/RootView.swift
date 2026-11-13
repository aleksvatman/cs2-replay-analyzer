import SwiftUI

/// Каркас навигации (этап I0): слева матчи и раунды, справа сводка или повтор.
struct RootView: View {
    var body: some View {
        NavigationSplitView {
            List {
                Text("Матчей пока нет")
                    .foregroundStyle(.secondary)
            }
            .navigationTitle("Матчи")
        } detail: {
            ContentUnavailableView(
                "Выберите матч",
                systemImage: "scope",
                description: Text("Здесь появятся сводка и повтор раунда.")
            )
        }
    }
}

#Preview {
    RootView()
}
